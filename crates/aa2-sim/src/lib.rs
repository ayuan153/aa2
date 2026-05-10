//! AA2 combat simulation crate.
//! Phase 0: deterministic tick-based autobattler combat with Dota2-style timing.

pub use aa2_data;

pub mod vec2;
pub mod unit;
pub mod projectile;
pub mod combat;
pub mod buff;
pub mod cast;
pub mod ability;
pub mod aoe;
pub mod ai;

use vec2::Vec2;
use unit::{Unit, UnitState, ACQUISITION_RANGE, ACTION_THRESHOLD};
use aa2_data::{HeroDef, UnitConfig};
use projectile::Projectile;
use combat::apply_armor;
use buff::{tick_buffs, active_status};
use cast::{tick_cooldowns, tick_cast, CastTickResult};

/// Simulation tick rate (ticks per second).
pub const TICK_RATE: f32 = 30.0;
/// Duration of one tick in seconds.
pub const TICK_DURATION: f32 = 1.0 / 30.0;

/// Tick counter type.
pub type Tick = u32;

/// Combat event for logging/replay.
#[derive(Debug, Clone, PartialEq)]
pub enum CombatEvent {
    /// A unit attacked and dealt damage (melee instant hit).
    Attack { tick: u32, attacker_id: u32, target_id: u32, damage: f32 },
    /// A ranged projectile was spawned.
    ProjectileSpawn { tick: u32, attacker_id: u32, target_id: u32 },
    /// A projectile hit its target.
    ProjectileHit { tick: u32, target_id: u32, damage: f32 },
    /// A unit died.
    Death { tick: u32, unit_id: u32 },
    /// The round ended with a winner.
    RoundEnd { tick: u32, winning_team: u8 },
    /// A buff was applied to a unit.
    BuffApplied { tick: u32, target_id: u32, name: String },
    /// A buff expired on a unit.
    BuffExpired { tick: u32, target_id: u32, name: String },
    /// A unit began casting an ability.
    CastStart { tick: u32, caster_id: u32, ability_name: String },
    /// A unit completed casting an ability.
    CastComplete { tick: u32, caster_id: u32, ability_name: String },
    /// An ability dealt damage to a target.
    AbilityDamage { tick: u32, caster_id: u32, target_id: u32, ability_name: String, damage: f32, damage_type: aa2_data::DamageType },
    /// A unit was healed by an ability.
    Heal { tick: u32, target_id: u32, amount: f32 },
}

/// Xoshiro128++ RNG for deterministic damage rolls.
/// We own this implementation to guarantee identical output across all platforms
/// and Rust versions (no external crate can change the algorithm under us).
#[derive(Debug, Clone)]
struct Rng {
    s: [u32; 4],
}

impl Rng {
    fn new(seed: u32) -> Self {
        // SplitMix32 to expand a single seed into 4 state words
        let mut z = seed;
        let mut s = [0u32; 4];
        for word in &mut s {
            z = z.wrapping_add(0x9e3779b9);
            let mut x = z;
            x = (x ^ (x >> 16)).wrapping_mul(0x21f0aaad);
            x = (x ^ (x >> 15)).wrapping_mul(0x735a2d97);
            x ^= x >> 15;
            *word = x;
        }
        Self { s }
    }

    /// Returns a random u32 using xoshiro128++.
    fn next_u32(&mut self) -> u32 {
        let result = (self.s[0].wrapping_add(self.s[3]))
            .rotate_left(7)
            .wrapping_add(self.s[0]);
        let t = self.s[1] << 9;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(11);
        result
    }

    /// Returns a uniform random f32 in [min, max].
    fn range_f32(&mut self, min: f32, max: f32) -> f32 {
        if min >= max { return min; }
        let t = (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32; // 24-bit precision
        min + (max - min) * t
    }

    /// Returns true with the given probability [0.0, 1.0].
    fn chance(&mut self, probability: f32) -> bool {
        let t = (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32;
        t < probability
    }
}

/// The core simulation state.
pub struct Simulation {
    /// All units in the simulation.
    pub units: Vec<Unit>,
    /// Active projectiles.
    pub projectiles: Vec<Projectile>,
    /// Current tick number.
    pub tick: Tick,
    /// Log of combat events.
    pub combat_log: Vec<CombatEvent>,
    /// Whether the simulation has ended.
    finished: bool,
    /// Winning team if finished.
    winner: Option<u8>,
    /// RNG for damage variance.
    rng: Rng,
}

/// Apply simple separation force to prevent unit stacking.
/// For each pair of living units within collision distance, push them apart.
pub fn apply_separation(units: &mut [Unit]) {
    let len = units.len();
    for i in 0..len {
        for j in (i + 1)..len {
            if !units[i].is_alive() || !units[j].is_alive() {
                continue;
            }
            let min_dist = units[i].collision_radius + units[j].collision_radius;
            let dist = units[i].position.distance(units[j].position);
            if dist < min_dist {
                let overlap = min_dist - dist;
                let dir = if dist > 1e-6 {
                    (units[j].position - units[i].position).normalize()
                } else {
                    // Perfectly overlapping: push apart using deterministic offset based on IDs
                    let angle = (i as f32) * 1.2566; // ~72 degrees apart
                    Vec2::new(angle.cos(), angle.sin())
                };
                let push = dir.scale(overlap * 0.5);
                units[i].position = units[i].position - push;
                units[j].position = units[j].position + push;
            }
        }
    }
}

impl Simulation {
    /// Create a new simulation with the given units.
    pub fn new(units: Vec<Unit>) -> Self {
        Self::with_seed(units, 42)
    }

    /// Create a new simulation with a specific RNG seed (for reproducibility).
    pub fn with_seed(units: Vec<Unit>, seed: u32) -> Self {
        Self {
            units,
            projectiles: Vec::new(),
            tick: 0,
            combat_log: Vec::new(),
            finished: false,
            winner: None,
            rng: Rng::new(seed),
        }
    }

    /// Create a 5v5 simulation from two teams of hero definitions.
    /// Team A is placed at y=0, team B at y=600, spread evenly along x=-200..200.
    pub fn new_5v5(team_a: &[HeroDef], team_b: &[HeroDef], seed: u32) -> Self {
        let mut units = Vec::new();
        let mut id = 0u32;
        for (i, def) in team_a.iter().enumerate() {
            let x = if team_a.len() == 1 { 0.0 } else { -200.0 + 400.0 * i as f32 / (team_a.len() - 1) as f32 };
            units.push(Unit::from_hero_def(def, id, 0, Vec2::new(x, 0.0)));
            id += 1;
        }
        for (i, def) in team_b.iter().enumerate() {
            let x = if team_b.len() == 1 { 0.0 } else { -200.0 + 400.0 * i as f32 / (team_b.len() - 1) as f32 };
            units.push(Unit::from_hero_def(def, id, 1, Vec2::new(x, 600.0)));
            id += 1;
        }
        Self::with_seed(units, seed)
    }

    /// Create a simulation from two teams of `UnitConfig`s.
    /// Team A at y=0, team B at y=600, spread along x=-200..200.
    pub fn from_configs(team_a: &[UnitConfig], team_b: &[UnitConfig], seed: u32) -> Self {
        let mut units = Vec::new();
        let mut id = 0u32;
        for (i, config) in team_a.iter().enumerate() {
            let x = if team_a.len() == 1 { 0.0 } else { -200.0 + 400.0 * i as f32 / (team_a.len() - 1) as f32 };
            units.push(Unit::from_config(config, id, 0, Vec2::new(x, 0.0)));
            id += 1;
        }
        for (i, config) in team_b.iter().enumerate() {
            let x = if team_b.len() == 1 { 0.0 } else { -200.0 + 400.0 * i as f32 / (team_b.len() - 1) as f32 };
            units.push(Unit::from_config(config, id, 1, Vec2::new(x, 600.0)));
            id += 1;
        }
        Self::with_seed(units, seed)
    }

    /// Whether the simulation has ended.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// The winning team, if any.
    pub fn winner(&self) -> Option<u8> {
        self.winner
    }

    /// Advance the simulation by one tick.
    pub fn step(&mut self) {
        if self.finished {
            return;
        }
        self.tick += 1;

        self.step_regen();
        self.step_buffs();
        self.step_casts();
        self.step_units();
        apply_separation(&mut self.units);
        self.step_projectiles();
        self.check_deaths();
        self.check_round_end();
    }

    fn step_regen(&mut self) {
        for unit in self.units.iter_mut() {
            if !unit.is_alive() { continue; }
            unit.hp = (unit.hp + unit.hp_regen * TICK_DURATION).min(unit.max_hp);
            unit.mana = (unit.mana + unit.mana_regen * TICK_DURATION).min(unit.max_mana);
        }
    }

    fn step_buffs(&mut self) {
        let tick = self.tick;
        let mut events = Vec::new();
        for unit in self.units.iter_mut() {
            if !unit.is_alive() { continue; }
            let result = tick_buffs(&mut unit.buffs);
            if result.damage > 0.0 {
                unit.hp -= result.damage;
            }
            if result.healing > 0.0 {
                unit.hp = (unit.hp + result.healing).min(unit.max_hp);
            }
            for name in result.expired {
                events.push(CombatEvent::BuffExpired { tick, target_id: unit.id, name });
            }
        }
        self.combat_log.extend(events);
    }

    fn step_casts(&mut self) {
        let tick = self.tick;
        let mut events = Vec::new();
        let unit_count = self.units.len();
        for i in 0..unit_count {
            if !self.units[i].is_alive() { continue; }
            // Tick cooldowns
            tick_cooldowns(&mut self.units[i].abilities);
            // Save cast target info before tick_cast potentially clears it
            let cast_target = self.units[i].cast_state.as_ref().map(|c| (c.target_id, c.target_pos));
            // Process active cast — split borrow by extracting cast_state temporarily
            let status = active_status(&self.units[i].buffs);
            let disabled = status.stunned || status.hexed;
            let mut cast_state = self.units[i].cast_state.take();
            let result = tick_cast(&mut cast_state, &self.units[i].abilities, disabled);
            self.units[i].cast_state = cast_state;
            match result {
                CastTickResult::Completed { ability_index, mana_cost } => {
                    self.units[i].mana -= mana_cost;
                    self.units[i].abilities[ability_index].cooldown_remaining = self.units[i].abilities[ability_index].def.cooldown;
                    let ability_def = self.units[i].abilities[ability_index].def.clone();
                    let level = self.units[i].abilities[ability_index].level;
                    let caster_id = self.units[i].id;
                    let caster_team = self.units[i].team;
                    let caster_pos = self.units[i].position;
                    let (target_id, target_pos) = cast_target.unwrap_or((None, None));

                    let name = ability_def.name.clone();
                    events.push(CombatEvent::CastComplete { tick, caster_id, ability_name: name });

                    // Execute ability effects
                    let ability_events = ability::execute_ability(
                        &ability_def, level, caster_id, caster_team, caster_pos,
                        target_id, target_pos, &mut self.units, tick,
                    );
                    events.extend(ability_events);

                    self.units[i].state = UnitState::Idle;
                }
                CastTickResult::Casting => {
                    self.units[i].state = UnitState::Casting;
                }
                CastTickResult::Interrupted => {
                    self.units[i].state = UnitState::Idle;
                }
                CastTickResult::None => {}
            }
        }
        self.combat_log.extend(events);
    }

    fn step_units(&mut self) {
        let mut new_projectiles: Vec<Projectile> = Vec::new();
        let mut events: Vec<CombatEvent> = Vec::new();

        let unit_count = self.units.len();
        for i in 0..unit_count {
            if !self.units[i].is_alive() { continue; }

            // Check status effects
            let status = active_status(&self.units[i].buffs);
            if status.stunned || status.hexed { continue; } // skip all actions

            // Skip units that are casting
            if self.units[i].cast_state.is_some() { continue; }

            // Try to cast an ability before falling through to auto-attack
            if let Some((ability_index, target_id, target_pos)) = ai::try_find_cast(&self.units[i], &self.units) {
                let cast_time = self.units[i].abilities[ability_index].def.cast_point;
                let ability_name = self.units[i].abilities[ability_index].def.name.clone();
                self.units[i].cast_state = Some(cast::CastInProgress {
                    ability_index,
                    target_id,
                    target_pos,
                    cast_time_remaining: cast_time,
                });
                self.units[i].state = UnitState::Casting;
                events.push(CombatEvent::CastStart {
                    tick: self.tick,
                    caster_id: self.units[i].id,
                    ability_name,
                });
                continue;
            }

            // Targeting
            self.update_target(i);

            let target_id = match self.units[i].target {
                Some(t) => t,
                None => {
                    self.units[i].state = UnitState::Idle;
                    continue;
                }
            };

            let target_idx = self.units.iter().position(|u| u.id == target_id).unwrap();
            let target_pos = self.units[target_idx].position;
            let unit = &self.units[i];
            let dist = unit.position.distance(target_pos);
            let angle_to_target = angle_diff(unit.facing, (target_pos - unit.position).angle());

            // Turning
            if angle_to_target.abs() >= ACTION_THRESHOLD {
                self.turn_toward(i, target_pos);
                self.units[i].state = UnitState::Turning;
                continue;
            }

            // If in attack cooldown, count down
            if self.units[i].state == UnitState::Attacking && self.units[i].attack_timer > 0.0 {
                self.units[i].attack_timer -= TICK_DURATION;
                if self.units[i].attack_timer <= 0.0 {
                    // Attack completes — deal damage or spawn projectile
                    let attacker_id = self.units[i].id;
                    let raw_dmg = self.rng.range_f32(self.units[i].damage_min, self.units[i].damage_max);
                    let is_melee = self.units[i].is_melee;
                    let proj_speed = self.units[i].projectile_speed.unwrap_or(900.0);
                    let attacker_pos = self.units[i].position;
                    let target_is_melee = self.units[target_idx].is_melee;
                    let armor = self.units[target_idx].armor;

                    // Damage block (innate melee: 50% chance to block 16)
                    let blocked = if target_is_melee && self.rng.chance(0.5) { 16.0_f32.min(raw_dmg) } else { 0.0 };
                    let after_block = raw_dmg - blocked;
                    let actual_dmg = apply_armor(after_block, armor);

                    if is_melee {
                        self.units[target_idx].hp -= actual_dmg;
                        events.push(CombatEvent::Attack {
                            tick: self.tick, attacker_id, target_id, damage: actual_dmg,
                        });
                    } else {
                        let proj = Projectile {
                            target_id,
                            damage: raw_dmg, // Store raw damage; block + armor applied on hit
                            position: attacker_pos,
                            speed: proj_speed,
                        };
                        new_projectiles.push(proj);
                        events.push(CombatEvent::ProjectileSpawn {
                            tick: self.tick, attacker_id, target_id,
                        });
                    }
                    // Set backswing/cooldown timer
                    let remaining = self.units[i].attack_interval - self.units[i].attack_point;
                    self.units[i].attack_timer = remaining;
                    self.units[i].state = UnitState::Idle; // ready for next cycle after cooldown
                }
                continue;
            }

            // Movement
            if dist > self.units[i].attack_range {
                if status.rooted { continue; } // cannot move
                self.move_toward(i, target_pos);
                self.units[i].state = UnitState::Moving;
                continue;
            }

            // In range, cooldown expired — begin attack
            if self.units[i].attack_timer <= 0.0 {
                if status.disarmed { continue; } // cannot attack
                self.units[i].state = UnitState::Attacking;
                self.units[i].attack_timer = self.units[i].attack_point;
            } else {
                // Counting down backswing cooldown
                self.units[i].attack_timer -= TICK_DURATION;
            }
        }

        self.projectiles.extend(new_projectiles);
        self.combat_log.extend(events);
    }

    fn update_target(&mut self, idx: usize) {
        let unit = &self.units[idx];
        // Check if current target is still valid
        if let Some(tid) = unit.target
            && let Some(t) = self.units.iter().find(|u| u.id == tid)
            && t.is_alive() && unit.position.distance(t.position) <= ACQUISITION_RANGE
        {
            return; // keep target
        }
        // Acquire new target: closest living enemy
        let unit_pos = self.units[idx].position;
        let unit_team = self.units[idx].team;
        let mut best: Option<(u32, f32)> = None;
        for other in self.units.iter() {
            if other.team == unit_team || !other.is_alive() { continue; }
            let d = unit_pos.distance(other.position);
            if d <= ACQUISITION_RANGE && (best.is_none() || d < best.unwrap().1) {
                best = Some((other.id, d));
            }
        }
        self.units[idx].target = best.map(|(id, _)| id);
    }

    fn turn_toward(&mut self, idx: usize, target_pos: Vec2) {
        let unit = &mut self.units[idx];
        let desired = (target_pos - unit.position).angle();
        let diff = angle_diff(unit.facing, desired);
        if diff.abs() <= unit.turn_rate {
            unit.facing = desired;
        } else {
            unit.facing += unit.turn_rate * diff.signum();
            unit.facing = normalize_angle(unit.facing);
        }
    }

    fn move_toward(&mut self, idx: usize, target_pos: Vec2) {
        let unit = &mut self.units[idx];
        let dir = (target_pos - unit.position).normalize();
        let step = unit.move_speed * TICK_DURATION;
        unit.position = unit.position + dir.scale(step);
        // Also update facing
        unit.facing = dir.angle();
    }

    fn step_projectiles(&mut self) {
        let mut hit_events: Vec<CombatEvent> = Vec::new();
        let mut to_remove: Vec<usize> = Vec::new();

        for (pi, proj) in self.projectiles.iter_mut().enumerate() {
            // Find target
            let target = self.units.iter().find(|u| u.id == proj.target_id);
            let target = match target {
                Some(t) if t.is_alive() => t,
                _ => { to_remove.push(pi); continue; }
            };

            let target_pos = target.position;
            let dist = proj.position.distance(target_pos);
            let travel = proj.speed * TICK_DURATION;

            if dist <= travel {
                // Hit — apply damage block and armor at impact
                let target_idx = self.units.iter().position(|u| u.id == proj.target_id).unwrap();
                let target_is_melee = self.units[target_idx].is_melee;
                let armor = self.units[target_idx].armor;
                let blocked = if target_is_melee && self.rng.chance(0.5) { 16.0_f32.min(proj.damage) } else { 0.0 };
                let actual_dmg = apply_armor(proj.damage - blocked, armor);
                self.units[target_idx].hp -= actual_dmg;
                hit_events.push(CombatEvent::ProjectileHit {
                    tick: self.tick, target_id: proj.target_id, damage: actual_dmg,
                });
                to_remove.push(pi);
            } else {
                let dir = (target_pos - proj.position).normalize();
                proj.position = proj.position + dir.scale(travel);
            }
        }

        // Remove in reverse order
        to_remove.sort_unstable();
        for idx in to_remove.into_iter().rev() {
            self.projectiles.swap_remove(idx);
        }
        self.combat_log.extend(hit_events);
    }

    fn check_deaths(&mut self) {
        for unit in self.units.iter_mut() {
            if unit.hp <= 0.0 && unit.state != UnitState::Dead {
                unit.state = UnitState::Dead;
                self.combat_log.push(CombatEvent::Death { tick: self.tick, unit_id: unit.id });
            }
        }
    }

    fn check_round_end(&mut self) {
        let team0_alive = self.units.iter().any(|u| u.team == 0 && u.is_alive());
        let team1_alive = self.units.iter().any(|u| u.team == 1 && u.is_alive());

        if !team0_alive || !team1_alive {
            self.finished = true;
            if team0_alive && !team1_alive {
                self.winner = Some(0);
                self.combat_log.push(CombatEvent::RoundEnd { tick: self.tick, winning_team: 0 });
            } else if team1_alive && !team0_alive {
                self.winner = Some(1);
                self.combat_log.push(CombatEvent::RoundEnd { tick: self.tick, winning_team: 1 });
            }
            // Both dead = draw, winner stays None
        }
    }
}

/// Compute shortest signed angle difference from `from` to `to` (in radians).
fn angle_diff(from: f32, to: f32) -> f32 {
    let mut d = to - from;
    while d > std::f32::consts::PI { d -= 2.0 * std::f32::consts::PI; }
    while d < -std::f32::consts::PI { d += 2.0 * std::f32::consts::PI; }
    d
}

/// Normalize angle to [-PI, PI].
fn normalize_angle(mut a: f32) -> f32 {
    while a > std::f32::consts::PI { a -= 2.0 * std::f32::consts::PI; }
    while a < -std::f32::consts::PI { a += 2.0 * std::f32::consts::PI; }
    a
}

#[cfg(test)]
mod tests {
    use super::*;
    use aa2_data::{Attribute, HeroDef};
    use unit::{derive_stats, compute_attack_interval};

    fn make_warrior() -> HeroDef {
        HeroDef {
            name: "Warrior".to_string(),
            primary_attribute: Attribute::Strength,
            base_str: 25.0,
            base_agi: 15.0,
            base_int: 10.0,
            str_gain: 3.0,
            agi_gain: 1.5,
            int_gain: 1.0,
            base_attack_time: 1.7,
            attack_range: 150.0,
            attack_point: 0.5,
            move_speed: 300.0,
            turn_rate: 0.6,
            collision_radius: 24.0,
            tier: 1,
            is_melee: true,
            base_damage_min: 28.0,
            base_damage_max: 32.0,
            projectile_speed: None,
        }
    }

    fn make_ranger() -> HeroDef {
        HeroDef {
            name: "Ranger".to_string(),
            primary_attribute: Attribute::Agility,
            base_str: 15.0,
            base_agi: 25.0,
            base_int: 10.0,
            str_gain: 1.5,
            agi_gain: 3.0,
            int_gain: 1.0,
            base_attack_time: 1.7,
            attack_range: 600.0,
            attack_point: 0.3,
            move_speed: 300.0,
            turn_rate: 0.6,
            collision_radius: 24.0,
            tier: 1,
            is_melee: false,
            base_damage_min: 23.0,
            base_damage_max: 27.0,
            projectile_speed: Some(900.0),
        }
    }

    #[test]
    fn test_simulation_step() {
        let mut sim = Simulation::new(vec![]);
        assert_eq!(sim.tick, 0);
        sim.step();
        assert_eq!(sim.tick, 1);
    }

    #[test]
    fn test_attribute_derivation() {
        let stats = derive_stats(25.0, 15.0, 10.0, &Attribute::Strength, 0.0, 28.0, 32.0);
        assert!((stats.max_hp - (120.0 + 25.0 * 22.0)).abs() < 0.01);
        assert!((stats.max_mana - (75.0 + 10.0 * 12.0)).abs() < 0.01);
        assert!((stats.hp_regen - (0.25 + 25.0 * 0.1)).abs() < 0.01);
        assert!((stats.mana_regen - (0.0 + 10.0 * 0.05)).abs() < 0.01);
        assert!((stats.armor - (0.0 + 15.0 * 0.167)).abs() < 0.01);
        assert!((stats.total_attack_speed - 115.0).abs() < 0.01);
        assert!((stats.damage_min - 53.0).abs() < 0.01); // base_min 28 + primary STR 25
        assert!((stats.damage_max - 57.0).abs() < 0.01); // base_max 32 + primary STR 25
    }

    #[test]
    fn test_attack_interval() {
        // BAT 1.7, total AS 115 -> interval = 1.7 / 1.15
        let interval = compute_attack_interval(1.7, 115.0);
        let expected = 1.7 / 1.15;
        assert!((interval - expected).abs() < 0.001);
    }

    #[test]
    fn test_armor_reduction() {
        use combat::damage_multiplier;
        // armor 0 -> multiplier = 1.0
        assert!((damage_multiplier(0.0) - 1.0).abs() < 0.001);
        // armor 10 -> 1 - (0.6) / (1 + 0.6) = 1 - 0.375 = 0.625
        assert!((damage_multiplier(10.0) - 0.625).abs() < 0.001);
        // armor -10 -> 1 - (-0.6) / (1 + 0.6) = 1 + 0.375 = 1.375
        assert!((damage_multiplier(-10.0) - 1.375).abs() < 0.001);
    }

    #[test]
    fn test_turn_rate() {
        // Unit facing 0 radians, target at PI radians away
        // turn_rate = 0.6 rad/tick
        // PI / 0.6 = ~5.24 -> 6 ticks to turn (since last step overshoots)
        let def = make_warrior();
        let mut unit = Unit::from_hero_def(&def, 0, 0, Vec2::new(0.0, 0.0));
        unit.facing = 0.0;
        // Target behind: angle PI
        let target_angle = std::f32::consts::PI;
        let mut ticks = 0;
        loop {
            let diff = angle_diff(unit.facing, target_angle);
            if diff.abs() < ACTION_THRESHOLD { break; }
            if diff.abs() <= unit.turn_rate {
                unit.facing = target_angle;
            } else {
                unit.facing += unit.turn_rate * diff.signum();
                unit.facing = normalize_angle(unit.facing);
            }
            ticks += 1;
            if ticks > 100 { panic!("turn took too long"); }
        }
        // PI / 0.6 = 5.24, minus threshold means ~5 ticks
        // With threshold 0.2007: need to turn PI - 0.2007 = 2.94 rad, 2.94/0.6 = 4.9 -> 5 ticks
        assert_eq!(ticks, 5);
    }

    #[test]
    fn test_melee_combat() {
        let def = make_warrior();
        let u0 = Unit::from_hero_def(&def, 0, 0, Vec2::new(0.0, 0.0));
        let u1 = Unit::from_hero_def(&def, 1, 1, Vec2::new(100.0, 0.0));

        let mut sim = Simulation::new(vec![u0, u1]);

        // Run until first attack event
        for _ in 0..300 {
            sim.step();
            if sim.combat_log.iter().any(|e| matches!(e, CombatEvent::Attack { .. })) {
                break;
            }
        }

        let first_attack = sim.combat_log.iter().find(|e| matches!(e, CombatEvent::Attack { .. }));
        assert!(first_attack.is_some(), "Expected an attack event");

        if let Some(CombatEvent::Attack { damage, .. }) = first_attack {
            // damage should be in range [damage_min, damage_max] * armor_multiplier
            let min_expected = apply_armor(53.0, sim.units[1].armor); // 28 + 25 STR
            let max_expected = apply_armor(57.0, sim.units[1].armor); // 32 + 25 STR
            assert!(*damage >= min_expected - 0.01 && *damage <= max_expected + 0.01,
                "Damage {damage} not in expected range [{min_expected}, {max_expected}]");
        }
    }

    #[test]
    fn test_ranged_combat() {
        let ranger_def = make_ranger();
        let warrior_def = make_warrior();
        let u0 = Unit::from_hero_def(&ranger_def, 0, 0, Vec2::new(0.0, 0.0));
        let u1 = Unit::from_hero_def(&warrior_def, 1, 1, Vec2::new(400.0, 0.0));

        let mut sim = Simulation::new(vec![u0, u1]);

        // Run until projectile spawn
        for _ in 0..300 {
            sim.step();
            if sim.combat_log.iter().any(|e| matches!(e, CombatEvent::ProjectileSpawn { .. })) {
                break;
            }
        }
        assert!(sim.combat_log.iter().any(|e| matches!(e, CombatEvent::ProjectileSpawn { .. })));

        let spawn_tick = sim.combat_log.iter().find_map(|e| {
            if let CombatEvent::ProjectileSpawn { tick, .. } = e { Some(*tick) } else { None }
        }).unwrap();

        // Continue until projectile hits
        for _ in 0..300 {
            sim.step();
            if sim.combat_log.iter().any(|e| matches!(e, CombatEvent::ProjectileHit { .. })) {
                break;
            }
        }

        let hit_tick = sim.combat_log.iter().find_map(|e| {
            if let CombatEvent::ProjectileHit { tick, .. } = e { Some(*tick) } else { None }
        }).unwrap();

        // Projectile travel time: distance / speed / TICK_DURATION ticks
        // Distance ~400, speed 900, travel time = 400/900 = 0.44s = ~13 ticks
        let travel_ticks = hit_tick - spawn_tick;
        assert!(travel_ticks > 5 && travel_ticks < 30, "Unexpected travel time: {travel_ticks}");
    }

    #[test]
    fn test_combat_to_death() {
        let def = make_warrior();
        let u0 = Unit::from_hero_def(&def, 0, 0, Vec2::new(0.0, 0.0));
        let u1 = Unit::from_hero_def(&def, 1, 1, Vec2::new(100.0, 0.0));

        let mut sim = Simulation::new(vec![u0, u1]);

        // Run until finished (max 3000 ticks = 100 seconds)
        for _ in 0..3000 {
            if sim.is_finished() { break; }
            sim.step();
        }

        assert!(sim.is_finished(), "Simulation should have ended");
        // With identical units, one should win (first mover advantage)
        assert!(sim.winner().is_some(), "Should have a winner");
        // Verify death event exists
        assert!(sim.combat_log.iter().any(|e| matches!(e, CombatEvent::Death { .. })));
        assert!(sim.combat_log.iter().any(|e| matches!(e, CombatEvent::RoundEnd { .. })));
    }

    #[test]
    fn test_stunned_unit_cannot_attack() {
        use crate::buff::{Buff, StackBehavior, DispelType, StatusFlags};

        let def = make_warrior();
        let mut u0 = Unit::from_hero_def(&def, 0, 0, Vec2::new(0.0, 0.0));
        let u1 = Unit::from_hero_def(&def, 1, 1, Vec2::new(100.0, 0.0));

        // Apply a 60-tick stun to unit 0
        u0.buffs.push(Buff {
            name: "stun".to_string(),
            remaining_ticks: 60,
            tick_effect: None,
            stacking: StackBehavior::RefreshDuration,
            dispel_type: DispelType::BasicDispel,
            status: StatusFlags { stunned: true, ..StatusFlags::default() },
            stat_modifier: None,
            source_id: 1,
        });

        let mut sim = Simulation::new(vec![u0, u1]);

        // Run for 60 ticks (stun duration)
        for _ in 0..60 {
            sim.step();
        }

        // Unit 0 should not have attacked during stun
        let u0_attacks = sim.combat_log.iter().filter(|e| {
            matches!(e, CombatEvent::Attack { attacker_id: 0, .. })
        }).count();
        assert_eq!(u0_attacks, 0, "Stunned unit should not attack");

        // Unit 1 should have attacked
        let u1_attacks = sim.combat_log.iter().filter(|e| {
            matches!(e, CombatEvent::Attack { attacker_id: 1, .. })
        }).count();
        assert!(u1_attacks > 0, "Non-stunned unit should attack");
    }

    #[test]
    fn test_unit_casts_ability() {
        use aa2_data::{AbilityDef, DamageType, Effect, TargetType};
        use crate::cast::AbilityState;

        let def = make_warrior();
        let mut u0 = Unit::from_hero_def(&def, 0, 0, Vec2::new(0.0, 0.0));
        let u1 = Unit::from_hero_def(&def, 1, 1, Vec2::new(100.0, 0.0));

        u0.abilities.push(AbilityState {
            def: AbilityDef {
                name: "Fireball".to_string(),
                cooldown: 10.0,
                mana_cost: 50.0,
                cast_point: 0.3,
                targeting: TargetType::SingleEnemy,
                effects: vec![Effect::Damage { kind: DamageType::Magical, base: vec![100.0] }],
                description: String::new(),
                aoe_shape: None,
                cast_range: 600.0,
            },
            cooldown_remaining: 0.0,
            level: 0,
        });

        let mut sim = Simulation::new(vec![u0, u1]);
        // Run a few ticks — unit should begin casting
        for _ in 0..5 {
            sim.step();
        }
        assert!(sim.combat_log.iter().any(|e| matches!(e, CombatEvent::CastStart { ability_name, .. } if ability_name == "Fireball")));
    }

    #[test]
    fn test_unit_prefers_ability_over_attack() {
        use aa2_data::{AbilityDef, DamageType, Effect, TargetType};
        use crate::cast::AbilityState;

        let def = make_warrior();
        let mut u0 = Unit::from_hero_def(&def, 0, 0, Vec2::new(0.0, 0.0));
        let u1 = Unit::from_hero_def(&def, 1, 1, Vec2::new(100.0, 0.0));

        u0.abilities.push(AbilityState {
            def: AbilityDef {
                name: "Smash".to_string(),
                cooldown: 10.0,
                mana_cost: 50.0,
                cast_point: 0.3,
                targeting: TargetType::SingleEnemy,
                effects: vec![Effect::Damage { kind: DamageType::Physical, base: vec![200.0] }],
                description: String::new(),
                aoe_shape: None,
                cast_range: 600.0,
            },
            cooldown_remaining: 0.0,
            level: 0,
        });

        let mut sim = Simulation::new(vec![u0, u1]);
        for _ in 0..5 {
            sim.step();
        }
        // Should have cast start but no attack
        let has_cast = sim.combat_log.iter().any(|e| matches!(e, CombatEvent::CastStart { .. }));
        let has_attack = sim.combat_log.iter().any(|e| matches!(e, CombatEvent::Attack { attacker_id: 0, .. }));
        assert!(has_cast, "Unit should cast ability");
        assert!(!has_attack, "Unit should not auto-attack when ability is ready");
    }

    #[test]
    fn test_unit_attacks_when_ability_on_cooldown() {
        use aa2_data::{AbilityDef, DamageType, Effect, TargetType};
        use crate::cast::AbilityState;

        let def = make_warrior();
        let mut u0 = Unit::from_hero_def(&def, 0, 0, Vec2::new(0.0, 0.0));
        let u1 = Unit::from_hero_def(&def, 1, 1, Vec2::new(100.0, 0.0));

        u0.abilities.push(AbilityState {
            def: AbilityDef {
                name: "Fireball".to_string(),
                cooldown: 10.0,
                mana_cost: 50.0,
                cast_point: 0.3,
                targeting: TargetType::SingleEnemy,
                effects: vec![Effect::Damage { kind: DamageType::Magical, base: vec![100.0] }],
                description: String::new(),
                aoe_shape: None,
                cast_range: 600.0,
            },
            cooldown_remaining: 5.0, // on cooldown
            level: 0,
        });

        let mut sim = Simulation::new(vec![u0, u1]);
        for _ in 0..60 {
            sim.step();
        }
        // Should have attacked, not cast
        let has_cast = sim.combat_log.iter().any(|e| matches!(e, CombatEvent::CastStart { .. }));
        let has_attack = sim.combat_log.iter().any(|e| matches!(e, CombatEvent::Attack { attacker_id: 0, .. }));
        assert!(!has_cast, "Unit should not cast when ability on cooldown");
        assert!(has_attack, "Unit should fall back to auto-attack");
    }

    #[test]
    fn test_unit_cannot_cast_when_silenced() {
        use aa2_data::{AbilityDef, DamageType, Effect, TargetType};
        use crate::cast::AbilityState;
        use crate::buff::{Buff, StackBehavior, DispelType, StatusFlags};

        let def = make_warrior();
        let mut u0 = Unit::from_hero_def(&def, 0, 0, Vec2::new(0.0, 0.0));
        let u1 = Unit::from_hero_def(&def, 1, 1, Vec2::new(100.0, 0.0));

        u0.abilities.push(AbilityState {
            def: AbilityDef {
                name: "Fireball".to_string(),
                cooldown: 10.0,
                mana_cost: 50.0,
                cast_point: 0.3,
                targeting: TargetType::SingleEnemy,
                effects: vec![Effect::Damage { kind: DamageType::Magical, base: vec![100.0] }],
                description: String::new(),
                aoe_shape: None,
                cast_range: 600.0,
            },
            cooldown_remaining: 0.0,
            level: 0,
        });

        u0.buffs.push(Buff {
            name: "silence".to_string(),
            remaining_ticks: 300, // long silence
            tick_effect: None,
            stacking: StackBehavior::RefreshDuration,
            dispel_type: DispelType::BasicDispel,
            status: StatusFlags { silenced: true, ..StatusFlags::default() },
            stat_modifier: None,
            source_id: 1,
        });

        let mut sim = Simulation::new(vec![u0, u1]);
        for _ in 0..60 {
            sim.step();
        }
        let has_cast = sim.combat_log.iter().any(|e| matches!(e, CombatEvent::CastStart { .. }));
        let has_attack = sim.combat_log.iter().any(|e| matches!(e, CombatEvent::Attack { attacker_id: 0, .. }));
        assert!(!has_cast, "Silenced unit should not cast");
        assert!(has_attack, "Silenced unit should still auto-attack");
    }

    #[test]
    fn test_unit_from_config() {
        use aa2_data::{AbilityDef, DamageType, Effect, TargetType, UnitConfig};

        let hero = make_warrior();
        let ability1 = AbilityDef {
            name: "Fireball".to_string(),
            cooldown: 10.0,
            mana_cost: 100.0,
            cast_point: 0.3,
            targeting: TargetType::SingleEnemy,
            effects: vec![Effect::Damage { kind: DamageType::Magical, base: vec![100.0, 150.0, 200.0] }],
            description: String::new(),
            aoe_shape: None,
            cast_range: 600.0,
        };
        let ability2 = AbilityDef {
            name: "War Cry".to_string(),
            cooldown: 30.0,
            mana_cost: 50.0,
            cast_point: 0.2,
            targeting: TargetType::NoTarget,
            effects: vec![Effect::ApplyBuff { name: "War Cry".to_string(), duration: 6.0 }],
            description: String::new(),
            aoe_shape: None,
            cast_range: 600.0,
        };

        let config = UnitConfig::new(hero)
            .with_ability(ability1, 2)
            .with_ability(ability2, 1);

        let unit = Unit::from_config(&config, 0, 0, Vec2::new(0.0, 0.0));
        assert_eq!(unit.abilities.len(), 2);
        assert_eq!(unit.abilities[0].def.name, "Fireball");
        assert_eq!(unit.abilities[0].level, 2);
        assert_eq!(unit.abilities[1].def.name, "War Cry");
        assert_eq!(unit.abilities[1].level, 1);
        assert_eq!(unit.abilities[0].cooldown_remaining, 0.0);
    }

    #[test]
    fn test_loadout_resolve() {
        use aa2_data::{load_loadout, resolve_loadout};
        use std::path::Path;

        let loadout = load_loadout(Path::new("../../data/loadouts/sven_nuker.ron")).unwrap();
        assert_eq!(loadout.hero, "sven");
        assert_eq!(loadout.abilities.len(), 2);
        assert_eq!(loadout.abilities[0], ("fireball".to_string(), 3));
        assert_eq!(loadout.abilities[1], ("war_cry".to_string(), 1));

        let config = resolve_loadout(&loadout, Path::new("../../data")).unwrap();
        assert_eq!(config.hero.name, "Sven");
        assert_eq!(config.abilities.len(), 2);
        assert_eq!(config.abilities[0].0.name, "Fireball");
        assert_eq!(config.abilities[0].1, 3);
        assert_eq!(config.abilities[1].0.name, "War Cry");
        assert_eq!(config.abilities[1].1, 1);
    }
}
