/// Compute damage multiplier from armor.
/// Formula: 1.0 - (0.06 * armor) / (1.0 + 0.06 * armor.abs())
pub fn damage_multiplier(armor: f32) -> f32 {
    1.0 - (0.06 * armor) / (1.0 + 0.06 * armor.abs())
}

/// Compute actual damage after armor reduction.
pub fn apply_armor(raw_damage: f32, armor: f32) -> f32 {
    raw_damage * damage_multiplier(armor)
}

/// Compute actual magical damage after magic resistance.
/// Magic resistance stacks multiplicatively: total = 1 - (1-base)(1-bonus1)(1-bonus2)...
pub fn apply_magic_resistance(raw_damage: f32, magic_resistance: f32) -> f32 {
    raw_damage * (1.0 - magic_resistance)
}
