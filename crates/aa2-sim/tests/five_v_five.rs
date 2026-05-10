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
