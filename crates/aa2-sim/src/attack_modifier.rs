//! Attack modifier system: passive abilities that trigger on auto-attack hits.
//!
//! Handles Fury Swipes (stacking damage), Chaos Strike (PRD crit + lifesteal),
//! and Essence Shift (stat steal). Integrates into the damage pipeline between
//! base damage roll and armor reduction.

use aa2_data::{Effect, value_at_level};
use crate::buff::{Buff, StackBehavior, DispelType, StatusFlags, StatModifier};
use crate::unit::Unit;
use crate::TICK_RATE;

/// PRD (Pseudo-Random Distribution) state for a proc-based ability.
#[derive(Debug, Clone)]
pub struct PrdState {
    /// The C constant for this proc chance.
    pub c_value: f32,
    /// Current accumulated chance (increases each non-proc).
    pub current_chance: f32,
}

impl PrdState {
    /// Create a new PRD state for the given nominal proc probability.
    pub fn new(nominal_chance: f32) -> Self {
        let c = prd_c_from_p(nominal_chance);
        Self { c_value: c, current_chance: c }
    }

    /// Roll the PRD using the given RNG. Returns true if proc'd.
    pub(crate) fn roll(&mut self, rng: &mut crate::Rng) -> bool {
        if rng.chance(self.current_chance) {
            self.current_chance = self.c_value;
            true
        } else {
            self.current_chance += self.c_value;
            false
        }
    }
}

/// Per-target attack modifier state (e.g. Fury Swipes stacks).
#[derive(Debug, Clone)]
pub struct TargetModifierState {
    /// Current Fury Swipes stacks on this target.
    pub fury_swipes_stacks: u32,
    /// Tick when stacks expire.
    pub fury_swipes_expiry_tick: u32,
}

/// Result of processing attack modifiers before armor.
#[derive(Debug, Clone)]
pub struct AttackResult {
    /// Final damage after crit and post-crit bonus (before armor).
    pub damage: f32,
    /// Crit multiplier applied (1.0 = no crit).
    pub crit_multiplier: f32,
    /// Lifesteal percentage from Chaos Strike (0.0 if no crit).
    pub lifesteal_pct: f32,
    /// Post-crit flat bonus (Fury Swipes).
    pub post_crit_bonus: f32,
}

/// Process attack modifiers when an attack lands. Called BEFORE armor reduction.
///
/// Pipeline: base_damage * crit_multiplier + post_crit_bonus (Fury Swipes).
pub(crate) fn process_attack_modifiers(
    attacker: &mut Unit,
    target_id: u32,
    base_damage: f32,
    tick: u32,
    rng: &mut crate::Rng,
    ally_chaos_strike: Option<(f32, f32, f32, f32)>, // (proc_chance, crit_min, crit_max, lifesteal)
) -> AttackResult {
    let mut crit_multiplier = 1.0_f32;
    let mut lifesteal_pct = 0.0_f32;
    let mut post_crit_bonus = 0.0_f32;

    let ability_count = attacker.abilities.len();
    for ai in 0..ability_count {
        let level = attacker.abilities[ai].level;
        if level == 0 { continue; }
        let effects = attacker.abilities[ai].def.effects.clone();
        for effect in &effects {
            match effect {
                Effect::ChaosStrike { proc_chance, crit_min, crit_max, lifesteal } => {
                    let chance = value_at_level(proc_chance, level);
                    let prd = get_or_create_prd(&mut attacker.prd_states, ai, chance);
                    if prd.roll(rng) {
                        let min_c = value_at_level(crit_min, level);
                        let max_c = value_at_level(crit_max, level);
                        let crit = rng.range_f32(min_c, max_c) / 100.0;
                        if crit > crit_multiplier {
                            crit_multiplier = crit;
                            lifesteal_pct = value_at_level(lifesteal, level) / 100.0;
                        }
                    }
                }
                Effect::FurySwipes { damage_per_stack, stack_duration, .. } => {
                    let duration_secs = value_at_level(stack_duration, level);
                    let stacks = get_fury_swipes_stacks(&attacker.attack_modifier_state, target_id, tick);
                    let dmg_per = value_at_level(damage_per_stack, level);
                    post_crit_bonus += stacks as f32 * dmg_per;
                    // Increment stacks after damage calc (post-hit)
                    let expiry = tick + (duration_secs * TICK_RATE) as u32;
                    set_fury_swipes_stacks(&mut attacker.attack_modifier_state, target_id, stacks + 1, expiry);
                }
                _ => {}
            }
        }
    }

    // Gaben aura: ally Chaos Strike
    if crit_multiplier <= 1.0
        && let Some((chance, cmin, cmax, ls)) = ally_chaos_strike
    {
        // Use a separate PRD roll inline (no persistent state for aura)
        if rng.chance(chance) {
            let crit = rng.range_f32(cmin, cmax) / 100.0;
            if crit > crit_multiplier {
                crit_multiplier = crit;
                lifesteal_pct = ls / 100.0;
            }
        }
    }

    let damage = base_damage * crit_multiplier + post_crit_bonus;
    AttackResult { damage, crit_multiplier, lifesteal_pct, post_crit_bonus }
}

/// Apply post-hit effects: Essence Shift stat steal, Chaos Strike lifesteal.
pub fn post_attack_effects(
    attacker: &mut Unit,
    target: &mut Unit,
    damage_dealt: f32,
    lifesteal_pct: f32,
    _tick: u32,
) {
    // Chaos Strike lifesteal
    if lifesteal_pct > 0.0 {
        let heal = damage_dealt * lifesteal_pct;
        attacker.hp = (attacker.hp + heal).min(attacker.max_hp);
    }

    // Essence Shift
    let ability_count = attacker.abilities.len();
    for ai in 0..ability_count {
        let level = attacker.abilities[ai].level;
        if level == 0 { continue; }
        let effects = attacker.abilities[ai].def.effects.clone();
        for effect in &effects {
            if let Effect::EssenceShift { str_steal, agi_steal, int_steal, agi_gain, duration } = effect {
                let dur_secs = value_at_level(duration, level);
                let dur_ticks = (dur_secs * TICK_RATE) as u32;
                let s_steal = value_at_level(str_steal, level);
                let a_steal = value_at_level(agi_steal, level);
                let _i_steal = value_at_level(int_steal, level);
                let a_gain = value_at_level(agi_gain, level);

                // Debuff on target: lose stats
                let debuff = Buff {
                    name: "essence_shift_debuff".to_string(),
                    remaining_ticks: dur_ticks,
                    tick_effect: None,
                    stacking: StackBehavior::Independent,
                    dispel_type: DispelType::Undispellable,
                    status: StatusFlags::default(),
                    stat_modifier: Some(StatModifier {
                        bonus_strength: -s_steal,
                        bonus_armor: -a_steal * 0.167,
                        ..StatModifier::default()
                    }),
                    source_id: attacker.id,
                    is_debuff: true,
                };
                target.buffs.push(debuff);

                // Buff on attacker: gain AGI
                let buff = Buff {
                    name: "essence_shift_buff".to_string(),
                    remaining_ticks: dur_ticks,
                    tick_effect: None,
                    stacking: StackBehavior::Independent,
                    dispel_type: DispelType::Undispellable,
                    status: StatusFlags::default(),
                    stat_modifier: Some(StatModifier {
                        bonus_armor: a_gain * 0.167,
                        bonus_attack_speed: a_gain,
                        ..StatModifier::default()
                    }),
                    source_id: attacker.id,
                    is_debuff: false,
                };
                attacker.buffs.push(buff);
            }
        }
    }

    // Fury Swipes armor reduction
    let ability_count = attacker.abilities.len();
    for ai in 0..ability_count {
        let level = attacker.abilities[ai].level;
        if level == 0 { continue; }
        let effects = attacker.abilities[ai].def.effects.clone();
        for effect in &effects {
            if let Effect::FurySwipes { armor_reduction_per_stack, stack_duration, .. } = effect {
                let armor_red = value_at_level(armor_reduction_per_stack, level);
                if armor_red > 0.0 {
                    let dur_secs = value_at_level(stack_duration, level);
                    let dur_ticks = (dur_secs * TICK_RATE) as u32;
                    let debuff = Buff {
                        name: "fury_swipes_armor".to_string(),
                        remaining_ticks: dur_ticks,
                        tick_effect: None,
                        stacking: StackBehavior::Independent,
                        dispel_type: DispelType::Undispellable,
                        status: StatusFlags::default(),
                        stat_modifier: Some(StatModifier {
                            bonus_armor: -armor_red,
                            ..StatModifier::default()
                        }),
                        source_id: attacker.id,
                        is_debuff: true,
                    };
                    target.buffs.push(debuff);
                }
            }
        }
    }
}

/// Find Chaos Strike aura parameters from allies within 1200 range.
/// Returns the best (highest proc chance) ally Chaos Strike params if any.
pub fn find_ally_chaos_strike_aura(
    attacker: &Unit,
    units: &[Unit],
) -> Option<(f32, f32, f32, f32)> {
    let mut best: Option<(f32, f32, f32, f32)> = None;
    for ally in units {
        if ally.id == attacker.id || ally.team != attacker.team || !ally.is_alive() {
            continue;
        }
        if attacker.position.distance(ally.position) > 1200.0 {
            continue;
        }
        for ability in &ally.abilities {
            if ability.level == 0 { continue; }
            for effect in &ability.def.effects {
                if let Effect::ChaosStrike { proc_chance, crit_min, crit_max, lifesteal } = effect {
                    let chance = value_at_level(proc_chance, ability.level) * 0.5;
                    let cmin = value_at_level(crit_min, ability.level);
                    let cmax = value_at_level(crit_max, ability.level);
                    let ls = value_at_level(lifesteal, ability.level);
                    if best.is_none() || chance > best.unwrap().0 {
                        best = Some((chance, cmin, cmax, ls));
                    }
                }
            }
        }
    }
    best
}

/// Get or create a PRD state for the given ability index.
fn get_or_create_prd(prd_states: &mut Vec<(usize, PrdState)>, ability_index: usize, nominal_chance: f32) -> &mut PrdState {
    if let Some(pos) = prd_states.iter().position(|(idx, _)| *idx == ability_index) {
        &mut prd_states[pos].1
    } else {
        prd_states.push((ability_index, PrdState::new(nominal_chance)));
        let last = prd_states.len() - 1;
        &mut prd_states[last].1
    }
}

/// Get current Fury Swipes stacks on a target (0 if expired or not found).
/// Fury Swipes Gaben: every 2 attacks on an enemy, apply 1 stack to all other enemies.
/// Call after post_attack_effects. Only triggers at level 9 (Gaben).
pub fn fury_swipes_gaben_spread(
    attacker: &mut Unit,
    target_id: u32,
    other_enemy_ids: &[u32],
    tick: u32,
) {
    // Check if attacker has Gaben Fury Swipes (level 9)
    let has_gaben_fs = attacker.abilities.iter().any(|a| {
        a.level >= 9 && a.def.effects.iter().any(|e| matches!(e, Effect::FurySwipes { .. }))
    });
    if !has_gaben_fs { return; }

    // Get current stacks on the target (already incremented this attack)
    let stacks = get_fury_swipes_stacks(&attacker.attack_modifier_state, target_id, tick);

    // Every 2 attacks (check if stack count is even)
    if stacks > 0 && stacks % 2 == 0 {
        let dur_ticks = attacker.abilities.iter()
            .filter_map(|a| {
                if a.level < 9 { return None; }
                a.def.effects.iter().find_map(|e| {
                    if let Effect::FurySwipes { stack_duration, .. } = e {
                        Some((value_at_level(stack_duration, a.level) * TICK_RATE) as u32)
                    } else { None }
                })
            })
            .next()
            .unwrap_or(600);

        let expiry = tick + dur_ticks;
        for &enemy_id in other_enemy_ids {
            if enemy_id == target_id { continue; }
            let existing = get_fury_swipes_stacks(&attacker.attack_modifier_state, enemy_id, tick);
            set_fury_swipes_stacks(&mut attacker.attack_modifier_state, enemy_id, existing + 1, expiry);
        }
    }
}

fn get_fury_swipes_stacks(state: &[(u32, TargetModifierState)], target_id: u32, tick: u32) -> u32 {
    state.iter()
        .find(|(id, _)| *id == target_id)
        .map(|(_, s)| if tick < s.fury_swipes_expiry_tick { s.fury_swipes_stacks } else { 0 })
        .unwrap_or(0)
}

/// Set Fury Swipes stacks for a target.
fn set_fury_swipes_stacks(state: &mut Vec<(u32, TargetModifierState)>, target_id: u32, stacks: u32, expiry: u32) {
    if let Some(entry) = state.iter_mut().find(|(id, _)| *id == target_id) {
        entry.1.fury_swipes_stacks = stacks;
        entry.1.fury_swipes_expiry_tick = expiry;
    } else {
        state.push((target_id, TargetModifierState {
            fury_swipes_stacks: stacks,
            fury_swipes_expiry_tick: expiry,
        }));
    }
}

/// Compute PRD C value from nominal probability P using binary search.
pub fn prd_c_from_p(p: f32) -> f32 {
    if p <= 0.0 { return 0.0; }
    if p >= 1.0 { return 1.0; }
    let mut c_low = 0.0_f32;
    let mut c_high = p;
    for _ in 0..30 {
        let c_mid = (c_low + c_high) / 2.0;
        let p_from_c = prd_p_from_c(c_mid);
        if p_from_c < p {
            c_low = c_mid;
        } else {
            c_high = c_mid;
        }
    }
    (c_low + c_high) / 2.0
}

/// Compute actual average proc probability from a given C value.
/// P = 1 / E[N] where E[N] = sum over n of n * P(proc on exactly attack n).
fn prd_p_from_c(c: f32) -> f32 {
    if c <= 0.0 { return 0.0; }
    if c >= 1.0 { return 1.0; }
    let max_n = (1.0 / c).ceil() as u32 + 1;
    let mut expected_n = 0.0_f32;
    let mut p_not_yet = 1.0_f32;
    for n in 1..=max_n {
        let p_this = (n as f32 * c).min(1.0);
        let p_proc_here = p_not_yet * p_this;
        expected_n += n as f32 * p_proc_here;
        p_not_yet *= 1.0 - p_this;
        if p_not_yet < 0.00001 { break; }
    }
    // Account for any remaining probability mass
    expected_n += (max_n + 1) as f32 * p_not_yet;
    1.0 / expected_n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rng;

    #[test]
    fn test_prd_c_value() {
        let c = prd_c_from_p(0.3333);
        // C should be significantly less than P (PRD property)
        assert!(c < 0.3333, "C should be less than P, got {c}");
        assert!(c > 0.10 && c < 0.20, "C for P=0.3333 should be ~0.14, got {c}");
    }

    #[test]
    fn test_prd_c_value_43() {
        let c = prd_c_from_p(0.4333);
        assert!(c < 0.4333, "C should be less than P, got {c}");
        assert!(c > 0.15 && c < 0.30, "C for P=0.4333 should be ~0.20-0.23, got {c}");
    }

    #[test]
    fn test_prd_converges() {
        let mut rng = Rng::new(12345);
        let mut prd = PrdState::new(0.3333);
        let mut procs = 0u32;
        let trials = 10000;
        for _ in 0..trials {
            if prd.roll(&mut rng) {
                procs += 1;
            }
        }
        let actual_rate = procs as f32 / trials as f32;
        assert!(
            (actual_rate - 0.3333).abs() < 0.02,
            "PRD actual rate {actual_rate} should be within 2% of 0.3333"
        );
    }

    #[test]
    fn test_fury_swipes_stacking() {
        use aa2_data::{AbilityDef, TargetType};
        use crate::cast::AbilityState;

        let mut rng = Rng::new(42);
        let ability = AbilityState {
            def: AbilityDef {
                name: "Fury Swipes".to_string(),
                cooldown: 0.0,
                mana_cost: vec![0.0],
                cast_point: 0.0,
                targeting: TargetType::Passive,
                effects: vec![Effect::FurySwipes {
                    damage_per_stack: vec![20.0],
                    stack_duration: vec![15.0],
                    armor_reduction_per_stack: vec![0.0],
                }],
                description: String::new(),
                aoe_shape: None,
                cast_range: 0.0,
            },
            cooldown_remaining: 0.0,
            level: 1,
            casts: 0,
        };

        let mut attacker = make_test_unit(0, 0);
        attacker.abilities.push(ability);
        let target_id = 1;
        let base_damage = 50.0;

        let mut damages = Vec::new();
        for tick in 0..5 {
            let result = process_attack_modifiers(&mut attacker, target_id, base_damage, tick, &mut rng, None);
            damages.push(result.damage);
        }

        // Hit 1: 0 stacks -> 50 + 0 = 50
        // Hit 2: 1 stack -> 50 + 20 = 70
        // Hit 3: 2 stacks -> 50 + 40 = 90
        // Hit 4: 3 stacks -> 50 + 60 = 110
        // Hit 5: 4 stacks -> 50 + 80 = 130
        assert!((damages[0] - 50.0).abs() < 0.01);
        assert!((damages[1] - 70.0).abs() < 0.01);
        assert!((damages[2] - 90.0).abs() < 0.01);
        assert!((damages[3] - 110.0).abs() < 0.01);
        assert!((damages[4] - 130.0).abs() < 0.01);
    }

    #[test]
    fn test_fury_swipes_no_crit() {
        use aa2_data::{AbilityDef, TargetType};
        use crate::cast::AbilityState;

        let mut rng = Rng::new(42);
        let mut attacker = make_test_unit(0, 0);
        // Add both Fury Swipes and a guaranteed crit
        attacker.abilities.push(AbilityState {
            def: AbilityDef {
                name: "Fury Swipes".to_string(),
                cooldown: 0.0,
                mana_cost: vec![0.0],
                cast_point: 0.0,
                targeting: TargetType::Passive,
                effects: vec![Effect::FurySwipes {
                    damage_per_stack: vec![20.0],
                    stack_duration: vec![15.0],
                    armor_reduction_per_stack: vec![0.0],
                }],
                description: String::new(),
                aoe_shape: None,
                cast_range: 0.0,
            },
            cooldown_remaining: 0.0,
            level: 1,
            casts: 0,
        });
        attacker.abilities.push(AbilityState {
            def: AbilityDef {
                name: "Chaos Strike".to_string(),
                cooldown: 0.0,
                mana_cost: vec![0.0],
                cast_point: 0.0,
                targeting: TargetType::Passive,
                effects: vec![Effect::ChaosStrike {
                    proc_chance: vec![1.0], // guaranteed crit
                    crit_min: vec![200.0],
                    crit_max: vec![200.0],
                    lifesteal: vec![0.0],
                }],
                description: String::new(),
                aoe_shape: None,
                cast_range: 0.0,
            },
            cooldown_remaining: 0.0,
            level: 1,
            casts: 0,
        });

        let target_id = 1;
        let base_damage = 50.0;

        // First attack: 0 FS stacks. Crit = 2.0x. Damage = 50*2 + 0 = 100
        let r1 = process_attack_modifiers(&mut attacker, target_id, base_damage, 0, &mut rng, None);
        assert!((r1.damage - 100.0).abs() < 0.01, "Expected 100, got {}", r1.damage);

        // Second attack: 1 FS stack. Crit = 2.0x. Damage = 50*2 + 20 = 120 (NOT 50+20)*2=140)
        let r2 = process_attack_modifiers(&mut attacker, target_id, base_damage, 1, &mut rng, None);
        assert!((r2.damage - 120.0).abs() < 0.01, "Expected 120, got {}", r2.damage);
    }

    #[test]
    fn test_chaos_strike_lifesteal() {
        use aa2_data::{AbilityDef, TargetType};
        use crate::cast::AbilityState;

        let mut rng = Rng::new(42);
        let mut attacker = make_test_unit(0, 0);
        attacker.hp = 400.0;
        attacker.max_hp = 600.0;

        attacker.abilities.push(AbilityState {
            def: AbilityDef {
                name: "Chaos Strike".to_string(),
                cooldown: 0.0,
                mana_cost: vec![0.0],
                cast_point: 0.0,
                targeting: TargetType::Passive,
                effects: vec![Effect::ChaosStrike {
                    proc_chance: vec![1.0], // guaranteed
                    crit_min: vec![200.0],
                    crit_max: vec![200.0],
                    lifesteal: vec![50.0], // 50%
                }],
                description: String::new(),
                aoe_shape: None,
                cast_range: 0.0,
            },
            cooldown_remaining: 0.0,
            level: 1,
            casts: 0,
        });

        let mut target = make_test_unit(1, 1);
        let base_damage = 50.0;
        let result = process_attack_modifiers(&mut attacker, target.id, base_damage, 0, &mut rng, None);

        // Damage dealt after armor would be less, but lifesteal is on pre-armor damage
        let damage_dealt = 60.0; // simulated post-armor
        post_attack_effects(&mut attacker, &mut target, damage_dealt, result.lifesteal_pct, 0);

        // 50% lifesteal on 60 damage = 30 heal
        assert!((attacker.hp - 430.0).abs() < 0.01, "Expected 430 HP, got {}", attacker.hp);
    }

    #[test]
    fn test_essence_shift_stat_steal() {
        use aa2_data::{AbilityDef, TargetType};
        use crate::cast::AbilityState;

        let mut attacker = make_test_unit(0, 0);
        let mut target = make_test_unit(1, 1);

        attacker.abilities.push(AbilityState {
            def: AbilityDef {
                name: "Essence Shift".to_string(),
                cooldown: 0.0,
                mana_cost: vec![0.0],
                cast_point: 0.0,
                targeting: TargetType::Passive,
                effects: vec![Effect::EssenceShift {
                    str_steal: vec![1.0],
                    agi_steal: vec![1.0],
                    int_steal: vec![1.0],
                    agi_gain: vec![3.0],
                    duration: vec![30.0],
                }],
                description: String::new(),
                aoe_shape: None,
                cast_range: 0.0,
            },
            cooldown_remaining: 0.0,
            level: 1,
            casts: 0,
        });

        post_attack_effects(&mut attacker, &mut target, 50.0, 0.0, 0);

        // Target should have a debuff reducing STR
        assert_eq!(target.buffs.len(), 1);
        assert_eq!(target.buffs[0].name, "essence_shift_debuff");
        let target_mod = target.buffs[0].stat_modifier.as_ref().unwrap();
        assert!((target_mod.bonus_strength - (-1.0)).abs() < 0.01);

        // Attacker should have a buff adding AGI-equivalent stats
        assert_eq!(attacker.buffs.len(), 1);
        assert_eq!(attacker.buffs[0].name, "essence_shift_buff");
        let attacker_mod = attacker.buffs[0].stat_modifier.as_ref().unwrap();
        assert!((attacker_mod.bonus_attack_speed - 3.0).abs() < 0.01);
    }

    fn make_test_unit(id: u32, team: u8) -> Unit {
        use aa2_data::{Attribute, HeroDef};
        use crate::vec2::Vec2;

        let def = HeroDef {
            name: "TestHero".to_string(),
            primary_attribute: Attribute::Strength,
            base_str: 20.0,
            base_agi: 20.0,
            base_int: 20.0,
            str_gain: 2.0,
            agi_gain: 2.0,
            int_gain: 2.0,
            base_attack_time: 1.7,
            attack_range: 150.0,
            attack_point: 0.5,
            move_speed: 300.0,
            turn_rate: 0.6,
            collision_radius: 24.0,
            tier: 1,
            is_melee: true,
            base_damage_min: 30.0,
            base_damage_max: 40.0,
            projectile_speed: None,
        };
        Unit::from_hero_def(&def, id, team, Vec2::new(0.0, 0.0))
    }
}
