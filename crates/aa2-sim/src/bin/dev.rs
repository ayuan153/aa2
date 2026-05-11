//! Dev-mode CLI binary: loads two heroes from RON files and runs combat to completion.

use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::process;

use aa2_sim::vec2::Vec2;
use aa2_sim::unit::Unit;
use aa2_sim::{CombatEvent, Simulation, TICK_RATE};

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}

/// Run the simulation and print results.
fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--5v5-loadout") {
        return run_5v5_loadout(&args);
    }
    if args.iter().any(|a| a == "--loadout") {
        return run_loadout(&args);
    }
    if args.iter().any(|a| a == "--5v5") {
        return run_5v5();
    }

    let hero1_path = args.get(1).map_or("data/heroes/warrior.ron", |s| s.as_str());
    let hero2_path = args.get(2).map_or("data/heroes/ranger.ron", |s| s.as_str());

    let def1 = aa2_sim::aa2_data::load_hero_def(Path::new(hero1_path))?;
    let def2 = aa2_sim::aa2_data::load_hero_def(Path::new(hero2_path))?;

    println!("=== AA2 Dev Combat ===");
    println!("{} (team 0) vs {} (team 1)\n", def1.name, def2.name);

    let u0 = Unit::from_hero_def(&def1, 0, 0, Vec2::new(0.0, 0.0));
    let u1 = Unit::from_hero_def(&def2, 1, 1, Vec2::new(500.0, 0.0));

    // Map unit IDs to names for readable output.
    let mut names: HashMap<u32, String> = HashMap::new();
    names.insert(0, def1.name.clone());
    names.insert(1, def2.name.clone());

    let mut sim = Simulation::new(vec![u0, u1]);
    let mut log_cursor = 0;

    while !sim.is_finished() {
        sim.step();
        // Print new events since last tick.
        for event in &sim.combat_log[log_cursor..] {
            print_event(event, &names, &sim.units);
        }
        log_cursor = sim.combat_log.len();
    }

    // Summary
    println!("\n=== Summary ===");
    println!("Total ticks: {} ({:.2}s)", sim.tick, sim.tick as f32 / TICK_RATE);
    if let Some(team) = sim.winner() {
        let winner = sim.units.iter().find(|u| u.team == team && u.is_alive());
        if let Some(w) = winner {
            let name = names.get(&w.id).map_or("Unknown", |n| n);
            println!("Winner: Team {team} ({name}) — {:.1}/{:.1} HP remaining", w.hp, w.max_hp);
        } else {
            println!("Winner: Team {team}");
        }
    } else {
        println!("Result: Draw (both teams eliminated)");
    }

    Ok(())
}

/// Run a 1v1 or 2-loadout simulation via `--loadout <file1> <file2>`.
fn run_loadout(args: &[String]) -> Result<(), String> {
    let loadout_idx = args.iter().position(|a| a == "--loadout").unwrap();
    let paths: Vec<&str> = args[loadout_idx + 1..].iter().map(|s| s.as_str()).collect();
    if paths.len() < 2 {
        return Err("--loadout requires at least 2 loadout file paths".to_string());
    }

    let data_dir = Path::new("data");
    let config_a = aa2_sim::aa2_data::resolve_loadout(
        &aa2_sim::aa2_data::load_loadout(Path::new(paths[0]))?, data_dir)?;
    let config_b = aa2_sim::aa2_data::resolve_loadout(
        &aa2_sim::aa2_data::load_loadout(Path::new(paths[1]))?, data_dir)?;

    println!("=== AA2 Dev Loadout Combat ===");
    println!("{} (team 0) vs {} (team 1)\n", config_a.hero.name, config_b.hero.name);

    let mut names: HashMap<u32, String> = HashMap::new();
    names.insert(0, config_a.hero.name.clone());
    names.insert(1, config_b.hero.name.clone());

    let mut sim = Simulation::from_configs(&[config_a], &[config_b], 42);
    run_sim(&mut sim, &names);
    Ok(())
}

/// Run a 5v5 loadout simulation via `--5v5-loadout <file1> ... <file10>`.
fn run_5v5_loadout(args: &[String]) -> Result<(), String> {
    let loadout_idx = args.iter().position(|a| a == "--5v5-loadout").unwrap();
    let paths: Vec<&str> = args[loadout_idx + 1..].iter().map(|s| s.as_str()).collect();
    if paths.len() < 2 || paths.len() > 10 {
        return Err("--5v5-loadout requires 2-10 loadout file paths (first half team A, second half team B)".to_string());
    }

    let data_dir = Path::new("data");
    let mid = paths.len() / 2;
    let mut team_a = Vec::new();
    let mut team_b = Vec::new();
    for p in &paths[..mid] {
        team_a.push(aa2_sim::aa2_data::resolve_loadout(
            &aa2_sim::aa2_data::load_loadout(Path::new(p))?, data_dir)?);
    }
    for p in &paths[mid..] {
        team_b.push(aa2_sim::aa2_data::resolve_loadout(
            &aa2_sim::aa2_data::load_loadout(Path::new(p))?, data_dir)?);
    }

    println!("=== AA2 Dev 5v5 Loadout Combat ===");
    println!("Team A: {}", team_a.iter().map(|c| c.hero.name.as_str()).collect::<Vec<_>>().join(", "));
    println!("Team B: {}\n", team_b.iter().map(|c| c.hero.name.as_str()).collect::<Vec<_>>().join(", "));

    let mut names: HashMap<u32, String> = HashMap::new();
    for (i, c) in team_a.iter().enumerate() {
        names.insert(i as u32, format!("{}[A]", c.hero.name));
    }
    for (i, c) in team_b.iter().enumerate() {
        names.insert((i + team_a.len()) as u32, format!("{}[B]", c.hero.name));
    }

    let mut sim = Simulation::from_configs(&team_a, &team_b, 42);
    run_sim(&mut sim, &names);
    Ok(())
}

/// Run a 5v5 simulation with all available heroes.
fn run_5v5() -> Result<(), String> {
    let heroes = aa2_sim::aa2_data::load_all_heroes(Path::new("data/heroes/"))?;
    if heroes.is_empty() {
        return Err("No hero files found in data/heroes/".to_string());
    }

    // Build teams: first 5 for A, next 5 for B (cycling if fewer than 10)
    let team_a: Vec<_> = (0..5).map(|i| heroes[i % heroes.len()].clone()).collect();
    let team_b: Vec<_> = (0..5).map(|i| heroes[(i + 5) % heroes.len()].clone()).collect();

    println!("=== AA2 Dev 5v5 Combat ===");
    println!("Team A: {}", team_a.iter().map(|h| h.name.as_str()).collect::<Vec<_>>().join(", "));
    println!("Team B: {}\n", team_b.iter().map(|h| h.name.as_str()).collect::<Vec<_>>().join(", "));

    let mut names: HashMap<u32, String> = HashMap::new();
    for (i, def) in team_a.iter().enumerate() {
        names.insert(i as u32, format!("{}[A]", def.name));
    }
    for (i, def) in team_b.iter().enumerate() {
        names.insert((i + 5) as u32, format!("{}[B]", def.name));
    }

    let mut sim = Simulation::new_5v5(&team_a, &team_b, 42);
    run_sim(&mut sim, &names);
    Ok(())
}

/// Run a simulation to completion and print events + summary.
fn run_sim(sim: &mut Simulation, names: &HashMap<u32, String>) {
    let mut log_cursor = 0;
    let max_ticks = 5000;
    for _ in 0..max_ticks {
        if sim.is_finished() { break; }
        sim.step();
        for event in &sim.combat_log[log_cursor..] {
            print_event(event, names, &sim.units);
        }
        log_cursor = sim.combat_log.len();
    }

    println!("\n=== Summary ===");
    println!("Total ticks: {} ({:.2}s)", sim.tick, sim.tick as f32 / TICK_RATE);
    if let Some(team) = sim.winner() {
        println!("Winner: Team {team}");
        println!("Survivors:");
        for unit in sim.units.iter().filter(|u| u.team == team && u.is_alive()) {
            let name = names.get(&unit.id).map_or("???", |n| n.as_str());
            println!("  {name}: {:.1}/{:.1} HP", unit.hp, unit.max_hp);
        }
    } else {
        println!("Result: Draw");
    }
}

/// Print a single combat event in human-readable format.
fn print_event(event: &CombatEvent, names: &HashMap<u32, String>, units: &[Unit]) {
    let name = |id: u32| names.get(&id).map_or("???", |n| n.as_str());
    match event {
        CombatEvent::Attack { tick, attacker_id, target_id, damage } => {
            let target = units.iter().find(|u| u.id == *target_id);
            let hp_after = target.map_or(0.0, |u| u.hp);
            let hp_before = hp_after + damage;
            println!("[tick {tick}] {} attacks {} for {damage:.1} damage (HP: {hp_before:.1} -> {hp_after:.1})",
                name(*attacker_id), name(*target_id));
        }
        CombatEvent::ProjectileSpawn { tick, attacker_id, target_id } => {
            println!("[tick {tick}] {} spawns projectile -> {}", name(*attacker_id), name(*target_id));
        }
        CombatEvent::ProjectileHit { tick, target_id, damage } => {
            let target = units.iter().find(|u| u.id == *target_id);
            let hp_after = target.map_or(0.0, |u| u.hp);
            let hp_before = hp_after + damage;
            println!("[tick {tick}] Projectile hits {} for {damage:.1} damage (HP: {hp_before:.1} -> {hp_after:.1})",
                name(*target_id));
        }
        CombatEvent::Death { tick, unit_id } => {
            println!("[tick {tick}] {} dies!", name(*unit_id));
        }
        CombatEvent::RoundEnd { tick, winning_team } => {
            let winner_name = units.iter()
                .find(|u| u.team == *winning_team && u.is_alive())
                .and_then(|u| names.get(&u.id))
                .map_or("???", |n| n.as_str());
            println!("[tick {tick}] === ROUND END: Team {winning_team} ({winner_name}) wins! ===");
        }
        CombatEvent::BuffApplied { tick, target_id, name: buff_name } => {
            println!("[tick {tick}] {} gains buff: {buff_name}", name(*target_id));
        }
        CombatEvent::BuffExpired { tick, target_id, name: buff_name } => {
            println!("[tick {tick}] {} loses buff: {buff_name}", name(*target_id));
        }
        CombatEvent::CastStart { tick, caster_id, ability_name } => {
            println!("[tick {tick}] {} begins casting {ability_name}", name(*caster_id));
        }
        CombatEvent::CastComplete { tick, caster_id, ability_name } => {
            println!("[tick {tick}] {} completes casting {ability_name}", name(*caster_id));
        }
        CombatEvent::AbilityDamage { tick, caster_id, target_id, ability_name, damage, damage_type } => {
            println!("[tick {tick}] {} hits {} with {ability_name} for {damage:.1} {damage_type:?} damage",
                name(*caster_id), name(*target_id));
        }
        CombatEvent::Heal { tick, target_id, amount } => {
            println!("[tick {tick}] {} healed for {amount:.1}", name(*target_id));
        }
        CombatEvent::DarkPactPulse { tick, caster_id, enemies_hit, self_damage } => {
            println!("[tick {tick}] {} Dark Pact pulse: {enemies_hit} enemies hit, {self_damage:.1} self-damage",
                name(*caster_id));
        }
        CombatEvent::WaveHit { tick, target_id, damage, stun_duration } => {
            println!("[tick {tick}] {} hit by wave for {damage:.1} damage, stunned {stun_duration:.2}s",
                name(*target_id));
        }
    }
}
