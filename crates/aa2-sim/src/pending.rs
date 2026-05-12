//! Pending effects system: delayed or over-time effects that fire independently of caster actions.

use aa2_data::DamageType;
use crate::vec2::Vec2;

/// The kind of pending effect currently active.
#[derive(Debug, Clone)]
pub enum PendingEffectKind {
    /// Burrowstrike: wave travels at speed, hitting units as it reaches them (capsule shape).
    BurrowstrikeTravel {
        start_pos: Vec2,
        end_pos: Vec2,
        travel_speed: f32,
        current_distance: f32,
        max_distance: f32,
        width: f32,
        damage: f32,
        stun_duration_secs: f32,
        caustic_finale_damage: f32,
        caustic_finale_radius: f32,
        caustic_finale_duration_secs: f32,
        already_hit: Vec<u32>,
        /// Pending damage: (unit_id, ticks_remaining, damage_amount)
        pending_damage: Vec<(u32, u32, f32)>,
    },
    /// Dark Pact: pulsing AoE damage + self-damage + self-dispel.
    DarkPactPulse {
        /// Damage dealt to each enemy per pulse.
        damage_per_pulse: f32,
        /// AoE radius around caster.
        radius: f32,
        /// Fraction of pulse damage dealt to self.
        self_damage_pct: f32,
        /// Damage type for the pulses.
        damage_type: DamageType,
        /// Whether each pulse applies strong dispel to self.
        dispel_self: bool,
        /// Whether self-damage cannot kill the caster.
        non_lethal: bool,
        /// Number of pulses remaining.
        pulses_remaining: u32,
        /// Ticks between pulses.
        pulse_interval_ticks: u32,
        /// Ticks until the next pulse fires.
        ticks_until_next_pulse: u32,
    },
    /// Ravage: expanding wave that stuns units as it reaches them.
    ExpandingWave {
        /// Magical damage dealt to each unit hit.
        damage: f32,
        /// Stun duration in seconds.
        stun_duration_secs: f32,
        /// Maximum radius the wave expands to.
        max_radius: f32,
        /// Speed of wave expansion in units/sec.
        wave_speed: f32,
        /// Current radius of the wave.
        current_radius: f32,
        /// Origin position (caster pos at cast time).
        origin: Vec2,
        /// Unit IDs already hit by this wave.
        already_hit: Vec<u32>,
    },
}

/// A pending effect that fires after a delay or over time.
#[derive(Debug, Clone)]
pub struct PendingEffect {
    /// ID of the caster who created this effect.
    pub caster_id: u32,
    /// Team of the caster.
    pub caster_team: u8,
    /// Name of the ability that created this effect.
    pub ability_name: String,
    /// The specific kind of pending effect.
    pub kind: PendingEffectKind,
    /// Ticks before the effect starts processing.
    pub delay_ticks_remaining: u32,
}
