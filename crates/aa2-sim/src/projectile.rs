use crate::vec2::Vec2;

/// A homing projectile in flight.
#[derive(Debug, Clone)]
pub struct Projectile {
    /// Target unit id.
    pub target_id: u32,
    /// Source attacker unit id (for post-hit effects).
    pub attacker_id: u32,
    /// Damage to deal on impact (after attack modifiers, before armor).
    pub damage: f32,
    /// Lifesteal percentage from attack modifiers (0.0 if none).
    pub lifesteal_pct: f32,
    /// Current world position.
    pub position: Vec2,
    /// Travel speed in units per second.
    pub speed: f32,
}
