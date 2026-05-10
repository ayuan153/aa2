use crate::vec2::Vec2;

/// A homing projectile in flight.
#[derive(Debug, Clone)]
pub struct Projectile {
    /// Target unit id.
    pub target_id: u32,
    /// Damage to deal on impact.
    pub damage: f32,
    /// Current world position.
    pub position: Vec2,
    /// Travel speed in units per second.
    pub speed: f32,
}
