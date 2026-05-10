//! Buff/debuff framework for the AA2 combat simulation.

use aa2_data::DamageType;

/// Behavior when the same buff is reapplied.
#[derive(Debug, Clone, PartialEq)]
pub enum StackBehavior {
    /// Same source reapplies → timer resets (default).
    RefreshDuration,
    /// Multiple instances accumulate up to max stacks.
    StackIntensity(u32),
    /// Each application is tracked separately.
    Independent,
}

/// Determines what strength of dispel can remove this buff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DispelType {
    /// Removed by any dispel.
    BasicDispel,
    /// Only removed by strong dispel.
    StrongDispel,
    /// Cannot be removed.
    Undispellable,
}

/// Status effect flags applied by buffs/debuffs.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct StatusFlags {
    /// Prevents all actions.
    pub stunned: bool,
    /// Prevents ability casting.
    pub silenced: bool,
    /// Prevents attacking.
    pub disarmed: bool,
    /// Prevents movement.
    pub rooted: bool,
    /// Prevents all actions + sets MS to 140 + disables passives.
    pub hexed: bool,
}

impl StatusFlags {
    /// Merge multiple status flags by OR-ing all fields together.
    pub fn merge(flags: &[StatusFlags]) -> StatusFlags {
        let mut result = StatusFlags::default();
        for f in flags {
            result.stunned |= f.stunned;
            result.silenced |= f.silenced;
            result.disarmed |= f.disarmed;
            result.rooted |= f.rooted;
            result.hexed |= f.hexed;
        }
        result
    }
}

/// Additive stat modifiers from buffs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StatModifier {
    /// Bonus armor (additive).
    pub bonus_armor: f32,
    /// Bonus attack speed (additive).
    pub bonus_attack_speed: f32,
    /// Bonus move speed (additive).
    pub bonus_move_speed: f32,
    /// Bonus damage (additive).
    pub bonus_damage: f32,
    /// Bonus magic resistance (multiplicative with base).
    pub bonus_magic_resistance: f32,
}

/// Periodic tick effect (DoT or HoT).
#[derive(Debug, Clone)]
pub struct TickEffect {
    /// Positive = damage, negative = heal.
    pub damage: f32,
    /// Type of damage dealt.
    pub damage_type: DamageType,
    /// Apply every N ticks.
    pub interval_ticks: u32,
    /// Countdown to next application.
    pub ticks_until_next: u32,
}

/// A buff or debuff applied to a unit.
#[derive(Debug, Clone)]
pub struct Buff {
    /// Name identifier for this buff.
    pub name: String,
    /// Ticks remaining before expiry.
    pub remaining_ticks: u32,
    /// Periodic tick effect (DoT/HoT).
    pub tick_effect: Option<TickEffect>,
    /// How this buff stacks with itself.
    pub stacking: StackBehavior,
    /// What dispel strength removes this buff.
    pub dispel_type: DispelType,
    /// Status effects this buff applies.
    pub status: StatusFlags,
    /// Stat modifiers this buff applies.
    pub stat_modifier: Option<StatModifier>,
    /// ID of the unit that applied this buff.
    pub source_id: u32,
}

/// Result of processing one tick of all buffs on a unit.
#[derive(Debug, Clone)]
pub struct BuffTickResult {
    /// Total tick damage to apply.
    pub damage: f32,
    /// Damage type of tick damage (defaults to Pure if no ticks fired).
    pub damage_type: DamageType,
    /// Total healing to apply.
    pub healing: f32,
    /// Names of buffs that expired this tick.
    pub expired: Vec<String>,
}

impl Default for BuffTickResult {
    fn default() -> Self {
        Self {
            damage: 0.0,
            damage_type: DamageType::Pure,
            healing: 0.0,
            expired: Vec::new(),
        }
    }
}

/// Apply a buff, handling stacking logic.
pub fn apply_buff(buffs: &mut Vec<Buff>, new_buff: Buff) {
    match &new_buff.stacking {
        StackBehavior::RefreshDuration => {
            if let Some(existing) = buffs.iter_mut().find(|b| b.name == new_buff.name && b.source_id == new_buff.source_id) {
                existing.remaining_ticks = new_buff.remaining_ticks;
                return;
            }
        }
        StackBehavior::StackIntensity(max) => {
            let count = buffs.iter().filter(|b| b.name == new_buff.name).count() as u32;
            if count >= *max {
                // Refresh oldest stack's duration
                if let Some(existing) = buffs.iter_mut().find(|b| b.name == new_buff.name) {
                    existing.remaining_ticks = new_buff.remaining_ticks;
                }
                return;
            }
        }
        StackBehavior::Independent => {} // always add
    }
    buffs.push(new_buff);
}

/// Tick all buffs: decrement timers, apply tick effects, remove expired.
pub fn tick_buffs(buffs: &mut Vec<Buff>) -> BuffTickResult {
    let mut result = BuffTickResult::default();

    for buff in buffs.iter_mut() {
        // Apply tick effects
        if let Some(ref mut effect) = buff.tick_effect {
            effect.ticks_until_next = effect.ticks_until_next.saturating_sub(1);
            if effect.ticks_until_next == 0 {
                if effect.damage > 0.0 {
                    result.damage += effect.damage;
                    result.damage_type = effect.damage_type.clone();
                } else if effect.damage < 0.0 {
                    result.healing += -effect.damage;
                }
                effect.ticks_until_next = effect.interval_ticks;
            }
        }
        buff.remaining_ticks = buff.remaining_ticks.saturating_sub(1);
    }

    // Remove expired and collect names
    let mut i = 0;
    while i < buffs.len() {
        if buffs[i].remaining_ticks == 0 {
            result.expired.push(buffs[i].name.clone());
            buffs.swap_remove(i);
        } else {
            i += 1;
        }
    }

    result
}

/// Get the merged status flags from all active buffs.
pub fn active_status(buffs: &[Buff]) -> StatusFlags {
    let flags: Vec<StatusFlags> = buffs.iter().map(|b| b.status).collect();
    StatusFlags::merge(&flags)
}

/// Remove all buffs that can be dispelled at the given strength.
pub fn dispel(buffs: &mut Vec<Buff>, strength: DispelType) {
    buffs.retain(|b| b.dispel_type > strength);
}

/// Sum all active stat modifiers.
pub fn total_stat_modifier(buffs: &[Buff]) -> StatModifier {
    let mut result = StatModifier::default();
    for buff in buffs {
        if let Some(ref m) = buff.stat_modifier {
            result.bonus_armor += m.bonus_armor;
            result.bonus_attack_speed += m.bonus_attack_speed;
            result.bonus_move_speed += m.bonus_move_speed;
            result.bonus_damage += m.bonus_damage;
            result.bonus_magic_resistance += m.bonus_magic_resistance;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_basic_buff(name: &str, ticks: u32, source: u32) -> Buff {
        Buff {
            name: name.to_string(),
            remaining_ticks: ticks,
            tick_effect: None,
            stacking: StackBehavior::RefreshDuration,
            dispel_type: DispelType::BasicDispel,
            status: StatusFlags::default(),
            stat_modifier: None,
            source_id: source,
        }
    }

    #[test]
    fn test_apply_buff_refresh() {
        let mut buffs = Vec::new();
        apply_buff(&mut buffs, make_basic_buff("slow", 30, 1));
        assert_eq!(buffs.len(), 1);
        assert_eq!(buffs[0].remaining_ticks, 30);

        apply_buff(&mut buffs, make_basic_buff("slow", 60, 1));
        assert_eq!(buffs.len(), 1);
        assert_eq!(buffs[0].remaining_ticks, 60);
    }

    #[test]
    fn test_apply_buff_stack() {
        let mut buffs = Vec::new();
        let mut b = make_basic_buff("poison", 30, 1);
        b.stacking = StackBehavior::StackIntensity(3);

        apply_buff(&mut buffs, b.clone());
        apply_buff(&mut buffs, b.clone());
        apply_buff(&mut buffs, b.clone());
        assert_eq!(buffs.len(), 3);

        // 4th should not add
        apply_buff(&mut buffs, b.clone());
        assert_eq!(buffs.len(), 3);
    }

    #[test]
    fn test_tick_effect() {
        let mut buffs = Vec::new();
        let mut b = make_basic_buff("dot", 90, 1);
        b.tick_effect = Some(TickEffect {
            damage: 10.0,
            damage_type: DamageType::Magical,
            interval_ticks: 30,
            ticks_until_next: 30,
        });
        buffs.push(b);

        // First 29 ticks: no damage
        for _ in 0..29 {
            let r = tick_buffs(&mut buffs);
            assert_eq!(r.damage, 0.0);
        }
        // 30th tick: damage fires
        let r = tick_buffs(&mut buffs);
        assert_eq!(r.damage, 10.0);
    }

    #[test]
    fn test_buff_expiry() {
        let mut buffs = Vec::new();
        buffs.push(make_basic_buff("shield", 3, 1));

        tick_buffs(&mut buffs);
        assert_eq!(buffs.len(), 1);
        tick_buffs(&mut buffs);
        assert_eq!(buffs.len(), 1);
        let r = tick_buffs(&mut buffs);
        assert_eq!(buffs.len(), 0);
        assert_eq!(r.expired, vec!["shield".to_string()]);
    }

    #[test]
    fn test_status_flags() {
        let mut buffs = Vec::new();
        let mut b = make_basic_buff("stun", 30, 1);
        b.status.stunned = true;
        buffs.push(b);

        let status = active_status(&buffs);
        assert!(status.stunned);
        assert!(!status.silenced);
    }

    #[test]
    fn test_dispel() {
        let mut buffs = Vec::new();
        buffs.push(make_basic_buff("basic_debuff", 30, 1));
        let mut strong = make_basic_buff("strong_debuff", 30, 1);
        strong.dispel_type = DispelType::StrongDispel;
        buffs.push(strong);

        dispel(&mut buffs, DispelType::BasicDispel);
        assert_eq!(buffs.len(), 1);
        assert_eq!(buffs[0].name, "strong_debuff");
    }

    #[test]
    fn test_stat_modifier() {
        let mut buffs = Vec::new();
        let mut b = make_basic_buff("armor_buff", 30, 1);
        b.stat_modifier = Some(StatModifier {
            bonus_armor: 5.0,
            ..StatModifier::default()
        });
        buffs.push(b);

        let mods = total_stat_modifier(&buffs);
        assert_eq!(mods.bonus_armor, 5.0);
        assert_eq!(mods.bonus_attack_speed, 0.0);
    }
}
