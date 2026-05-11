//! Ability execution engine: resolves ability effects when a cast completes.

use aa2_data::{AbilityDef, DamageType, Effect, TargetType};
use crate::aoe::find_aoe_targets;
use crate::buff::{apply_buff, Buff, DispelType, StackBehavior, StatModifier, StatusFlags};
use crate::combat::{apply_armor, apply_magic_resistance};
use crate::pending::{PendingEffect, PendingEffectKind};
use crate::unit::Unit;
use crate::vec2::Vec2;
use crate::CombatEvent;

/// Execute an ability's effects when cast completes.
/// Returns a list of combat events generated.
#[allow(clippy::too_many_arguments)]
pub fn execute_ability(
    ability: &AbilityDef,
    level: u8,
    caster_id: u32,
    caster_team: u8,
    caster_pos: Vec2,
    target_id: Option<u32>,
    target_pos: Option<Vec2>,
    units: &mut [Unit],
    tick: u32,
    pending_effects: &mut Vec<PendingEffect>,
) -> Vec<CombatEvent> {
    let mut events = Vec::new();

    // Determine target indices based on targeting type
    let target_indices: Vec<usize> = match &ability.targeting {
        TargetType::PointAoE => {
            let Some(shape) = &ability.aoe_shape else { return events };
            let origin = target_pos.unwrap_or(caster_pos);
            let direction = (origin - caster_pos).normalize();
            // Default to facing right if origin == caster_pos
            let direction = if direction.length() < 1e-6 { Vec2::new(1.0, 0.0) } else { direction };
            // Damage effects hit enemies, heal effects hit allies — use first effect to decide
            let hit_enemies = ability.effects.first().is_none_or(|e| !matches!(e, Effect::Heal { .. }));
            find_aoe_targets(shape, origin, direction, units, caster_id, caster_team, hit_enemies)
        }
        TargetType::NoTarget => {
            // No target needed — effects handled in the second loop (DarkPact, etc.)
            vec![]
        }
        _ => {
            // Single-target: resolve target
            match target_id {
                Some(tid) => match units.iter().position(|u| u.id == tid && u.is_alive()) {
                    Some(idx) => vec![idx],
                    None => return events,
                },
                None => return events,
            }
        }
    };

    for &idx in &target_indices {
        for effect in &ability.effects {
            match effect {
                Effect::Damage { kind, base } => {
                    let raw = value_at_level(base, level);
                    let actual = match kind {
                        DamageType::Physical => apply_armor(raw, units[idx].armor),
                        DamageType::Magical => apply_magic_resistance(raw, units[idx].magic_resistance),
                        DamageType::Pure => raw,
                    };
                    units[idx].hp -= actual;
                    events.push(CombatEvent::AbilityDamage {
                        tick,
                        caster_id,
                        target_id: units[idx].id,
                        ability_name: ability.name.clone(),
                        damage: actual,
                        damage_type: kind.clone(),
                    });
                }
                Effect::Heal { base } => {
                    let raw = value_at_level(base, level);
                    let before = units[idx].hp;
                    units[idx].hp = (units[idx].hp + raw).min(units[idx].max_hp);
                    let healed = units[idx].hp - before;
                    events.push(CombatEvent::Heal {
                        tick,
                        target_id: units[idx].id,
                        amount: healed,
                    });
                }
                Effect::ApplyBuff { name, duration } => {
                    let buff = Buff {
                        name: name.clone(),
                        remaining_ticks: (*duration * 30.0) as u32,
                        tick_effect: None,
                        stacking: StackBehavior::RefreshDuration,
                        dispel_type: DispelType::BasicDispel,
                        status: StatusFlags::default(),
                        stat_modifier: None,
                        source_id: caster_id,
                    };
                    apply_buff(&mut units[idx].buffs, buff);
                    events.push(CombatEvent::BuffApplied {
                        tick,
                        target_id: units[idx].id,
                        name: name.clone(),
                    });
                }
                Effect::Summon { .. } => {}
                Effect::DarkPact { .. } | Effect::BuffTargetAndSelf { .. } | Effect::ExpandingWaveStun { .. } => {
                    // These are handled outside the per-target loop
                }
            }
        }
    }

    // Handle effects that don't iterate over targets
    for effect in &ability.effects {
        match effect {
            Effect::DarkPact {
                kind, total_damage, radius, self_damage_pct,
                delay, pulse_count, pulse_interval, dispel_self, non_lethal,
            } => {
                let dmg_total = value_at_level(total_damage, level);
                let r = value_at_level(radius, level);
                let interval_ticks = (*pulse_interval * 30.0) as u32;
                pending_effects.push(PendingEffect {
                    caster_id,
                    caster_team,
                    ability_name: ability.name.clone(),
                    kind: PendingEffectKind::DarkPactPulse {
                        damage_per_pulse: dmg_total / *pulse_count as f32,
                        radius: r,
                        self_damage_pct: *self_damage_pct,
                        damage_type: kind.clone(),
                        dispel_self: *dispel_self,
                        non_lethal: *non_lethal,
                        pulses_remaining: *pulse_count,
                        pulse_interval_ticks: interval_ticks,
                        ticks_until_next_pulse: 0,
                    },
                    delay_ticks_remaining: (*delay * 30.0) as u32,
                });
            }
            Effect::BuffTargetAndSelf {
                name, duration, hp_regen, strength, status_resistance,
            } => {
                let dur_ticks = (value_at_level(duration, level) * 30.0) as u32;
                let modifier = StatModifier {
                    bonus_hp_regen: value_at_level(hp_regen, level),
                    bonus_strength: value_at_level(strength, level),
                    status_resistance: value_at_level(status_resistance, level),
                    ..StatModifier::default()
                };
                let make_buff = || Buff {
                    name: name.clone(),
                    remaining_ticks: dur_ticks,
                    tick_effect: None,
                    stacking: StackBehavior::RefreshDuration,
                    dispel_type: DispelType::BasicDispel,
                    status: StatusFlags::default(),
                    stat_modifier: Some(modifier.clone()),
                    source_id: caster_id,
                };
                // Apply to target
                if let Some(tid) = target_id
                    && let Some(target) = units.iter_mut().find(|u| u.id == tid && u.is_alive())
                {
                    target.status_resistance += modifier.status_resistance;
                    apply_buff(&mut target.buffs, make_buff());
                    events.push(CombatEvent::BuffApplied { tick, target_id: tid, name: name.clone() });
                }
                // Apply to caster
                if let Some(caster) = units.iter_mut().find(|u| u.id == caster_id && u.is_alive()) {
                    caster.status_resistance += modifier.status_resistance;
                    apply_buff(&mut caster.buffs, make_buff());
                    events.push(CombatEvent::BuffApplied { tick, target_id: caster_id, name: name.clone() });
                }
            }
            Effect::ExpandingWaveStun {
                damage, stun_duration, radius, wave_speed,
            } => {
                pending_effects.push(PendingEffect {
                    caster_id,
                    caster_team,
                    ability_name: ability.name.clone(),
                    kind: PendingEffectKind::ExpandingWave {
                        damage: value_at_level(damage, level),
                        stun_duration_secs: value_at_level(stun_duration, level),
                        max_radius: value_at_level(radius, level),
                        wave_speed: *wave_speed,
                        current_radius: 0.0,
                        origin: caster_pos,
                        already_hit: Vec::new(),
                    },
                    delay_ticks_remaining: 0,
                });
            }
            _ => {}
        }
    }

    events
}

/// Get value from a per-level array. Level is 1-indexed (level 1 = base[0]).
fn value_at_level(base: &[f32], level: u8) -> f32 {
    let idx = (level.saturating_sub(1) as usize).min(base.len().saturating_sub(1));
    base[idx]
}

#[cfg(test)]
mod tests {
    use super::*;
    use aa2_data::{AbilityDef, Attribute, DamageType, Effect, HeroDef, TargetType};
    use crate::unit::Unit;
    use crate::vec2::Vec2;

    fn make_test_hero() -> HeroDef {
        HeroDef {
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
            base_damage_max: 30.0,
            projectile_speed: None,
        }
    }

    fn make_ability(effects: Vec<Effect>) -> AbilityDef {
        AbilityDef {
            name: "TestAbility".to_string(),
            cooldown: 10.0,
            mana_cost: 100.0,
            cast_point: 0.3,
            targeting: TargetType::SingleEnemy,
            effects,
            description: String::new(),
            aoe_shape: None,
            cast_range: 600.0,
        }
    }

    #[test]
    fn test_ability_damage_physical() {
        let def = make_test_hero();
        let mut units = vec![
            Unit::from_hero_def(&def, 0, 0, Vec2::new(0.0, 0.0)),
            Unit::from_hero_def(&def, 1, 1, Vec2::new(100.0, 0.0)),
        ];
        let ability = make_ability(vec![Effect::Damage {
            kind: DamageType::Physical,
            base: vec![100.0, 150.0, 200.0],
        }]);

        let hp_before = units[1].hp;
        let armor = units[1].armor;
        let events = execute_ability(&ability, 1, 0, 0, Vec2::new(0.0, 0.0), Some(1), None, &mut units, 10, &mut Vec::new());

        let expected_dmg = apply_armor(100.0, armor);
        assert!((hp_before - units[1].hp - expected_dmg).abs() < 0.01);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], CombatEvent::AbilityDamage { damage, damage_type: DamageType::Physical, .. } if (*damage - expected_dmg).abs() < 0.01));
    }

    #[test]
    fn test_ability_damage_magical() {
        let def = make_test_hero();
        let mut units = vec![
            Unit::from_hero_def(&def, 0, 0, Vec2::new(0.0, 0.0)),
            Unit::from_hero_def(&def, 1, 1, Vec2::new(100.0, 0.0)),
        ];
        let ability = make_ability(vec![Effect::Damage {
            kind: DamageType::Magical,
            base: vec![200.0],
        }]);

        let hp_before = units[1].hp;
        let mr = units[1].magic_resistance; // 0.25
        execute_ability(&ability, 1, 0, 0, Vec2::new(0.0, 0.0), Some(1), None, &mut units, 10, &mut Vec::new());

        let expected_dmg = apply_magic_resistance(200.0, mr);
        assert!((hp_before - units[1].hp - expected_dmg).abs() < 0.01);
        // 25% magic resistance -> 150 damage
        assert!((expected_dmg - 150.0).abs() < 0.01);
    }

    #[test]
    fn test_ability_damage_pure() {
        let def = make_test_hero();
        let mut units = vec![
            Unit::from_hero_def(&def, 0, 0, Vec2::new(0.0, 0.0)),
            Unit::from_hero_def(&def, 1, 1, Vec2::new(100.0, 0.0)),
        ];
        let ability = make_ability(vec![Effect::Damage {
            kind: DamageType::Pure,
            base: vec![100.0],
        }]);

        let hp_before = units[1].hp;
        execute_ability(&ability, 1, 0, 0, Vec2::new(0.0, 0.0), Some(1), None, &mut units, 10, &mut Vec::new());

        assert!((hp_before - units[1].hp - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_ability_heal() {
        let def = make_test_hero();
        let mut units = vec![
            Unit::from_hero_def(&def, 0, 0, Vec2::new(0.0, 0.0)),
            Unit::from_hero_def(&def, 1, 1, Vec2::new(100.0, 0.0)),
        ];
        // Damage the target first
        units[1].hp = 100.0;
        let ability = make_ability(vec![Effect::Heal { base: vec![50.0, 75.0, 100.0] }]);

        let events = execute_ability(&ability, 2, 0, 0, Vec2::new(0.0, 0.0), Some(1), None, &mut units, 10, &mut Vec::new());

        assert!((units[1].hp - 175.0).abs() < 0.01);
        assert!(matches!(&events[0], CombatEvent::Heal { amount, .. } if (*amount - 75.0).abs() < 0.01));

        // Test cap at max_hp
        units[1].hp = units[1].max_hp - 10.0;
        let events = execute_ability(&ability, 2, 0, 0, Vec2::new(0.0, 0.0), Some(1), None, &mut units, 20, &mut Vec::new());
        assert!((units[1].hp - units[1].max_hp).abs() < 0.01);
        assert!(matches!(&events[0], CombatEvent::Heal { amount, .. } if (*amount - 10.0).abs() < 0.01));
    }

    #[test]
    fn test_ability_apply_buff() {
        let def = make_test_hero();
        let mut units = vec![
            Unit::from_hero_def(&def, 0, 0, Vec2::new(0.0, 0.0)),
            Unit::from_hero_def(&def, 1, 1, Vec2::new(100.0, 0.0)),
        ];
        let ability = make_ability(vec![Effect::ApplyBuff {
            name: "slow".to_string(),
            duration: 3.0,
        }]);

        let events = execute_ability(&ability, 1, 0, 0, Vec2::new(0.0, 0.0), Some(1), None, &mut units, 10, &mut Vec::new());

        assert_eq!(units[1].buffs.len(), 1);
        assert_eq!(units[1].buffs[0].name, "slow");
        assert_eq!(units[1].buffs[0].remaining_ticks, 90); // 3.0 * 30
        assert!(matches!(&events[0], CombatEvent::BuffApplied { name, .. } if name == "slow"));
    }
}
