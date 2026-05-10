use aa2_data::{Attribute, HeroDef};
use crate::vec2::Vec2;

/// Base HP added to all units before attribute scaling.
pub const BASE_HP: f32 = 120.0;
/// Base mana added to all units before attribute scaling.
pub const BASE_MANA: f32 = 75.0;
/// Base HP regen per second before attribute scaling.
pub const BASE_HP_REGEN: f32 = 0.25;
/// Base mana regen per second before attribute scaling.
pub const BASE_MANA_REGEN: f32 = 0.0;
/// Base armor before attribute scaling.
pub const BASE_ARMOR: f32 = 0.0;
/// Base attack damage before primary attribute bonus.
pub const BASE_DAMAGE: f32 = 0.0;
/// Acquisition range for targeting enemies.
pub const ACQUISITION_RANGE: f32 = 800.0;
/// Angle threshold (radians) below which a unit can act toward its target.
pub const ACTION_THRESHOLD: f32 = 0.2007;

/// Unit behavioral state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnitState {
    /// Doing nothing.
    Idle,
    /// Rotating to face target.
    Turning,
    /// Walking toward target.
    Moving,
    /// In attack animation (frontswing or cooldown).
    Attacking,
    /// Dead.
    Dead,
}

/// A combat unit in the simulation.
#[derive(Debug, Clone)]
pub struct Unit {
    /// Unique identifier.
    pub id: u32,
    /// Team index (0 or 1).
    pub team: u8,
    /// Current hit points.
    pub hp: f32,
    /// Maximum hit points.
    pub max_hp: f32,
    /// Current mana.
    pub mana: f32,
    /// Maximum mana.
    pub max_mana: f32,
    /// HP regeneration per second.
    pub hp_regen: f32,
    /// Mana regeneration per second.
    pub mana_regen: f32,
    /// Armor value (can be negative).
    pub armor: f32,
    /// Attack damage per hit.
    pub attack_damage: f32,
    /// Time between attacks in seconds.
    pub attack_interval: f32,
    /// Effective frontswing duration in seconds.
    pub attack_point: f32,
    /// Attack range in game units.
    pub attack_range: f32,
    /// Movement speed in units per second.
    pub move_speed: f32,
    /// Turn rate in radians per tick.
    pub turn_rate: f32,
    /// World position.
    pub position: Vec2,
    /// Facing direction in radians.
    pub facing: f32,
    /// Collision radius.
    pub collision_radius: f32,
    /// Whether this unit is melee.
    pub is_melee: bool,
    /// Projectile speed for ranged units.
    pub projectile_speed: Option<f32>,
    /// Current behavioral state.
    pub state: UnitState,
    /// Timer counting down during attack animation or cooldown.
    pub attack_timer: f32,
    /// Current target unit id.
    pub target: Option<u32>,
}

/// Derived combat stats from attributes.
pub struct DerivedStats {
    pub max_hp: f32,
    pub max_mana: f32,
    pub hp_regen: f32,
    pub mana_regen: f32,
    pub armor: f32,
    pub total_attack_speed: f32,
    pub attack_damage: f32,
}

/// Derive combat stats from STR/AGI/INT and bonus attack speed.
pub fn derive_stats(str_val: f32, agi_val: f32, int_val: f32, primary: &Attribute, bonus_as: f32) -> DerivedStats {
    let primary_val = match primary {
        Attribute::Strength => str_val,
        Attribute::Agility => agi_val,
        Attribute::Intelligence => int_val,
    };
    DerivedStats {
        max_hp: BASE_HP + str_val * 22.0,
        max_mana: BASE_MANA + int_val * 12.0,
        hp_regen: BASE_HP_REGEN + str_val * 0.1,
        mana_regen: BASE_MANA_REGEN + int_val * 0.05,
        armor: BASE_ARMOR + agi_val * 0.167,
        total_attack_speed: (100.0 + agi_val + bonus_as).clamp(20.0, 700.0),
        attack_damage: BASE_DAMAGE + primary_val,
    }
}

/// Compute attack interval from BAT and total attack speed.
pub fn compute_attack_interval(bat: f32, total_attack_speed: f32) -> f32 {
    bat / (total_attack_speed / 100.0)
}

/// Compute effective attack point (frontswing) from base attack point and total attack speed.
pub fn compute_effective_attack_point(base_attack_point: f32, total_attack_speed: f32) -> f32 {
    base_attack_point * (100.0 / total_attack_speed)
}

impl Unit {
    /// Create a Unit from a HeroDef, team, position, and unique id.
    /// Uses base attributes at level 1 with no items.
    pub fn from_hero_def(def: &HeroDef, id: u32, team: u8, position: Vec2) -> Self {
        let stats = derive_stats(def.base_str, def.base_agi, def.base_int, &def.primary_attribute, 0.0);
        let attack_interval = compute_attack_interval(def.base_attack_time, stats.total_attack_speed);
        let attack_point = compute_effective_attack_point(def.attack_point, stats.total_attack_speed);
        let projectile_speed = if def.is_melee { None } else { Some(900.0) }; // default ranged projectile speed

        Self {
            id,
            team,
            hp: stats.max_hp,
            max_hp: stats.max_hp,
            mana: stats.max_mana,
            max_mana: stats.max_mana,
            hp_regen: stats.hp_regen,
            mana_regen: stats.mana_regen,
            armor: stats.armor,
            attack_damage: stats.attack_damage,
            attack_interval,
            attack_point,
            attack_range: def.attack_range,
            move_speed: def.move_speed,
            turn_rate: def.turn_rate,
            position,
            facing: if team == 0 { 0.0 } else { std::f32::consts::PI },
            collision_radius: def.collision_radius,
            is_melee: def.is_melee,
            projectile_speed,
            state: UnitState::Idle,
            attack_timer: 0.0,
            target: None,
        }
    }

    /// Whether this unit is alive.
    pub fn is_alive(&self) -> bool {
        self.state != UnitState::Dead && self.hp > 0.0
    }
}
