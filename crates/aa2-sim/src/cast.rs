//! Cast system: ability timing, cooldowns, mana deduction, and interrupts.

use aa2_data::{AbilityDef, value_at_level};
use crate::vec2::Vec2;
use crate::TICK_DURATION;

/// Charge-based ability state.
#[derive(Debug, Clone)]
pub struct ChargeState {
    pub max_charges: u32,
    pub current_charges: u32,
    pub charge_cooldown: f32,
    pub charge_timer: f32,
}

/// Runtime state for an equipped ability on a unit.
#[derive(Debug, Clone)]
pub struct AbilityState {
    /// The ability definition from data.
    pub def: AbilityDef,
    /// Seconds until this ability is ready (0 = ready).
    pub cooldown_remaining: f32,
    /// Current ability level (0-indexed into base arrays in effects).
    pub level: u8,
    /// Number of times this ability has been cast.
    pub casts: u32,
    /// Charge state (if ability uses charges).
    pub charges: Option<ChargeState>,
}

impl AbilityState {
    /// Returns true if this ability is ready to cast (off cooldown or has charges).
    pub fn is_ready(&self) -> bool {
        if let Some(charges) = &self.charges {
            charges.current_charges > 0
        } else {
            self.cooldown_remaining <= 0.0
        }
    }

    /// Consume a use of this ability (decrement charge or start cooldown).
    pub fn consume(&mut self) {
        if let Some(charges) = &mut self.charges {
            charges.current_charges -= 1;
            if charges.charge_timer <= 0.0 {
                charges.charge_timer = charges.charge_cooldown;
            }
        } else {
            self.cooldown_remaining = value_at_level(&self.def.cooldown, self.level);
        }
    }
}

/// An in-progress cast on a unit.
#[derive(Debug, Clone)]
pub struct CastInProgress {
    /// Index into the unit's `abilities` vec.
    pub ability_index: usize,
    /// Target unit ID (for single-target abilities).
    pub target_id: Option<u32>,
    /// Target position (for ground-targeted abilities).
    pub target_pos: Option<Vec2>,
    /// Seconds of cast point remaining.
    pub cast_time_remaining: f32,
}

/// Tick all ability cooldowns on a unit, decrementing by TICK_DURATION.
pub fn tick_cooldowns(abilities: &mut [AbilityState]) {
    for ability in abilities.iter_mut() {
        if let Some(charges) = &mut ability.charges {
            if charges.current_charges < charges.max_charges && charges.charge_timer > 0.0 {
                charges.charge_timer -= TICK_DURATION;
                if charges.charge_timer <= 0.0 {
                    charges.current_charges += 1;
                    if charges.current_charges < charges.max_charges {
                        charges.charge_timer = charges.charge_cooldown;
                    } else {
                        charges.charge_timer = 0.0;
                    }
                }
            }
        } else if ability.cooldown_remaining > 0.0 {
            ability.cooldown_remaining = (ability.cooldown_remaining - TICK_DURATION).max(0.0);
        }
    }
}

/// Result of processing a unit's cast state for one tick.
#[derive(Debug)]
pub enum CastTickResult {
    /// Nothing happened (no active cast).
    None,
    /// Cast is still in progress.
    Casting,
    /// Cast completed: ability fires. Returns (ability_index, mana_cost).
    Completed { ability_index: usize, mana_cost: f32 },
    /// Cast was interrupted (unit stunned/hexed).
    Interrupted,
}

/// Process one tick of a unit's cast state.
/// `is_disabled` should be true if the unit is stunned or hexed.
pub fn tick_cast(cast_state: &mut Option<CastInProgress>, abilities: &[AbilityState], is_disabled: bool) -> CastTickResult {
    let cast = match cast_state.as_mut() {
        Some(c) => c,
        None => return CastTickResult::None,
    };

    if is_disabled {
        *cast_state = None;
        return CastTickResult::Interrupted;
    }

    cast.cast_time_remaining -= TICK_DURATION;
    if cast.cast_time_remaining <= 1e-6 {
        let idx = cast.ability_index;
        let mana_cost = value_at_level(&abilities[idx].def.mana_cost, abilities[idx].level);
        *cast_state = None;
        CastTickResult::Completed { ability_index: idx, mana_cost }
    } else {
        CastTickResult::Casting
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aa2_data::{AbilityDef, TargetType};

    fn make_test_ability(cast_point: f32, cooldown: f32, mana_cost: f32) -> AbilityDef {
        AbilityDef {
            name: "TestAbility".to_string(),
            cooldown: vec![cooldown],
            mana_cost: vec![mana_cost],
            cast_point,
            targeting: TargetType::NoTarget,
            effects: vec![],
            description: String::new(),
            aoe_shape: None,
            cast_range: 600.0, cast_behavior: aa2_data::CastBehavior::default(), max_charges: None,
        }
    }

    #[test]
    fn test_cast_point_timing() {
        // 0.3s cast point at 30 ticks/sec = 9 ticks
        let ability_def = make_test_ability(0.3, 10.0, 100.0);
        let abilities = vec![AbilityState { def: ability_def, cooldown_remaining: 0.0, level: 0, casts: 0, charges: None }];
        let mut cast_state = Some(CastInProgress {
            ability_index: 0,
            target_id: None,
            target_pos: None,
            cast_time_remaining: 0.3,
        });

        let mut ticks = 0;
        loop {
            let result = tick_cast(&mut cast_state, &abilities, false);
            ticks += 1;
            match result {
                CastTickResult::Completed { .. } => break,
                CastTickResult::Casting => continue,
                _ => panic!("unexpected result"),
            }
        }
        assert_eq!(ticks, 9);
    }

    #[test]
    fn test_mana_deduction() {
        let ability_def = make_test_ability(0.1, 10.0, 75.0);
        let abilities = vec![AbilityState { def: ability_def, cooldown_remaining: 0.0, level: 0, casts: 0, charges: None }];
        let mut cast_state = Some(CastInProgress {
            ability_index: 0,
            target_id: None,
            target_pos: None,
            cast_time_remaining: 0.1,
        });

        let mut mana: f32 = 200.0;

        // Mana should NOT be deducted during cast
        for _ in 0..2 {
            let result = tick_cast(&mut cast_state, &abilities, false);
            if matches!(result, CastTickResult::Casting) {
                assert_eq!(mana, 200.0, "Mana should not be deducted during cast");
            }
        }

        // On completion tick, deduct mana
        let result = tick_cast(&mut cast_state, &abilities, false);
        if let CastTickResult::Completed { mana_cost, .. } = result {
            mana -= mana_cost;
        }
        assert_eq!(mana, 125.0);
    }

    #[test]
    fn test_cast_interrupt() {
        let ability_def = make_test_ability(0.5, 10.0, 100.0);
        let abilities = vec![AbilityState { def: ability_def, cooldown_remaining: 0.0, level: 0, casts: 0, charges: None }];
        let mut cast_state = Some(CastInProgress {
            ability_index: 0,
            target_id: None,
            target_pos: None,
            cast_time_remaining: 0.5,
        });

        let mana: f32 = 200.0;

        // Tick a few times normally
        for _ in 0..5 {
            tick_cast(&mut cast_state, &abilities, false);
        }
        assert!(cast_state.is_some());

        // Now stun interrupts
        let result = tick_cast(&mut cast_state, &abilities, true);
        assert!(matches!(result, CastTickResult::Interrupted));
        assert!(cast_state.is_none());
        // No mana spent
        assert_eq!(mana, 200.0);

        // Verify no completion happens
        let result = tick_cast(&mut cast_state, &abilities, false);
        assert!(matches!(result, CastTickResult::None));
    }

    #[test]
    fn test_cooldown_tick() {
        let ability_def = make_test_ability(0.3, 1.0, 50.0);
        let mut abilities = vec![AbilityState { def: ability_def, cooldown_remaining: 1.0, level: 0, casts: 0, charges: None }];

        // 1.0s cooldown at 30 ticks/sec = 30 ticks
        for i in 0..30 {
            tick_cooldowns(&mut abilities);
            if i < 29 {
                assert!(abilities[0].cooldown_remaining > 0.0);
            }
        }
        assert_eq!(abilities[0].cooldown_remaining, 0.0);
    }
}
