/// Compute damage multiplier from armor.
/// Formula: 1.0 - (0.06 * armor) / (1.0 + 0.06 * armor.abs())
pub fn damage_multiplier(armor: f32) -> f32 {
    1.0 - (0.06 * armor) / (1.0 + 0.06 * armor.abs())
}

/// Compute actual damage after armor reduction.
pub fn apply_armor(raw_damage: f32, armor: f32) -> f32 {
    raw_damage * damage_multiplier(armor)
}
