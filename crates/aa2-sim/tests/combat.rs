//! Multi-unit combat tests: targeting, movement, separation, and turn rate mechanics.

use std::path::Path;

use aa2_sim::Simulation;

fn load_heroes() -> Vec<aa2_data::HeroDef> {
    aa2_data::load_all_heroes(Path::new("../../data/heroes/")).unwrap()
}

#[test]
fn test_5v5_combat() {
    let heroes = load_heroes();
    // Use first 5 for team A, duplicate/wrap for team B
    let team_a: Vec<_> = heroes.iter().take(5).cloned().collect();
    let team_b: Vec<_> = heroes.iter().skip(5).chain(heroes.iter()).take(5).cloned().collect();

    let mut sim = Simulation::new_5v5(&team_a, &team_b, 123);

    let max_ticks = 5000;
    for _ in 0..max_ticks {
        if sim.is_finished() {
            break;
        }
        sim.step();
    }

    assert!(sim.is_finished(), "Simulation should complete within {max_ticks} ticks");
    assert!(sim.winner().is_some(), "Should have a winner");

    let winning_team = sim.winner().unwrap();
    let losing_team = 1 - winning_team;

    // All units on losing team should be dead
    for unit in &sim.units {
        if unit.team == losing_team {
            assert!(!unit.is_alive(), "All losing team units should be dead");
        }
    }

    // At least one unit on winning team should be alive
    assert!(
        sim.units.iter().any(|u| u.team == winning_team && u.is_alive()),
        "Winning team should have at least one survivor"
    );
}

#[test]
fn test_separation_prevents_stacking() {
    let heroes = load_heroes();
    let def = &heroes[0];

    // Create units manually at the same position
    let mut units: Vec<aa2_sim::unit::Unit> = (0..5)
        .map(|i| aa2_sim::unit::Unit::from_hero_def(def, i, 0, aa2_sim::vec2::Vec2::new(0.0, 0.0)))
        .collect();

    // Apply separation directly
    aa2_sim::apply_separation(&mut units);

    // After separation, not all units should be at the same spot
    let positions: Vec<_> = units.iter().map(|u| u.position).collect();
    let all_same = positions.windows(2).all(|w| w[0].distance(w[1]) < 1.0);
    assert!(!all_same, "Units should have been pushed apart by separation");
}

/// Test that Io (instant turn rate) casts faster than a slow-turning hero
/// when the target is behind them (requiring a 180° turn).
#[test]
fn test_io_instant_turn_rate_vs_slow_hero() {
    use aa2_sim::vec2::Vec2;
    use aa2_sim::unit::Unit;
    use aa2_sim::cast::AbilityState;
    use aa2_sim::{Simulation, CombatEvent};
    use aa2_data::{AbilityDef, DamageType, Effect, TargetType};

    // Create a simple targeted ability
    let ability = AbilityDef {
        name: "Test Bolt".to_string(),
        cooldown: 30.0,
        mana_cost: vec![50.0],
        cast_point: 0.3,
        targeting: TargetType::SingleEnemy,
        effects: vec![Effect::Damage { kind: DamageType::Magical, base: vec![100.0] }],
        description: String::new(),
        aoe_shape: None,
        cast_range: 600.0,
    };

    // Io: instant turn rate (999.0)
    let io_def = aa2_data::load_hero_def(std::path::Path::new("../../data/heroes/io.ron")).unwrap();
    // Slow hero: turn rate 0.5 (needs ~6 ticks to turn 180°)
    let mut slow_def = io_def.clone();
    slow_def.name = "SlowTurner".to_string();
    slow_def.turn_rate = 0.5;

    // Place heroes facing UP (positive Y), with enemy BEHIND them (negative Y)
    // This forces a ~180° turn before casting
    let mut io_unit = Unit::from_hero_def(&io_def, 0, 0, Vec2::new(0.0, 300.0));
    io_unit.facing = std::f32::consts::FRAC_PI_2; // facing up (+Y)
    io_unit.abilities.push(AbilityState { def: ability.clone(), cooldown_remaining: 0.0, level: 1, casts: 0 });

    let mut slow_unit = Unit::from_hero_def(&slow_def, 2, 0, Vec2::new(200.0, 300.0));
    slow_unit.facing = std::f32::consts::FRAC_PI_2; // facing up (+Y)
    slow_unit.abilities.push(AbilityState { def: ability.clone(), cooldown_remaining: 0.0, level: 1, casts: 0 });

    // Enemy behind both heroes (at Y=0, within cast range 600)
    let enemy_def = io_def.clone();
    let enemy = Unit::from_hero_def(&enemy_def, 1, 1, Vec2::new(100.0, 0.0));

    // Run Io simulation
    let mut sim_io = Simulation::new(vec![io_unit, enemy.clone()]);
    let mut io_cast_tick = None;
    for _ in 0..100 {
        sim_io.step();
        if io_cast_tick.is_none() {
            if let Some(evt) = sim_io.combat_log.iter().find(|e| matches!(e, CombatEvent::CastStart { .. })) {
                if let CombatEvent::CastStart { tick, .. } = evt {
                    io_cast_tick = Some(*tick);
                }
            }
        }
        if io_cast_tick.is_some() { break; }
    }

    // Run slow hero simulation
    let enemy2 = Unit::from_hero_def(&enemy_def, 1, 1, Vec2::new(100.0, 0.0));
    let mut sim_slow = Simulation::new(vec![slow_unit, enemy2]);
    let mut slow_cast_tick = None;
    for _ in 0..100 {
        sim_slow.step();
        if slow_cast_tick.is_none() {
            if let Some(evt) = sim_slow.combat_log.iter().find(|e| matches!(e, CombatEvent::CastStart { .. })) {
                if let CombatEvent::CastStart { tick, .. } = evt {
                    slow_cast_tick = Some(*tick);
                }
            }
        }
        if slow_cast_tick.is_some() { break; }
    }

    let io_tick = io_cast_tick.expect("Io should have started casting");
    let slow_tick = slow_cast_tick.expect("SlowTurner should have started casting");

    println!("Io cast start: tick {io_tick}");
    println!("SlowTurner cast start: tick {slow_tick}");
    println!("Difference: {} ticks ({:.2}s)", slow_tick - io_tick, (slow_tick - io_tick) as f32 / 30.0);

    // Io should cast on tick 1 (instant turn, immediately faces target)
    // SlowTurner needs ~6 ticks to turn 180° at 0.5 rad/tick (PI / 0.5 = 6.28 ticks)
    assert!(io_tick < slow_tick, "Io (instant turn) should cast before slow hero");
    assert!(slow_tick - io_tick >= 5, "Slow hero should need at least 5 ticks to turn 180°");
}
