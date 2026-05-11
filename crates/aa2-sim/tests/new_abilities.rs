//! Tests for Dark Pact, Heavenly Grace, and Ravage mechanics.

use aa2_data::{AbilityDef, Attribute, DamageType, Effect, HeroDef, TargetType, UnitConfig};
use aa2_sim::buff::{Buff, DispelType, StackBehavior, StatusFlags};
use aa2_sim::unit::Unit;
use aa2_sim::vec2::Vec2;
use aa2_sim::{CombatEvent, Simulation};

fn make_hero() -> HeroDef {
    HeroDef {
        name: "TestHero".to_string(),
        primary_attribute: Attribute::Strength,
        base_str: 20.0,
        base_agi: 20.0,
        base_int: 20.0,
        str_gain: 2.0,
        agi_gain: 2.0,
        int_gain: 2.0,
        base_attack_time: 1.7,
        attack_range: 150.0,
        attack_point: 0.5,
        move_speed: 300.0,
        turn_rate: 0.6,
        collision_radius: 24.0,
        tier: 1,
        is_melee: true,
        base_damage_min: 30.0,
        base_damage_max: 30.0,
        projectile_speed: None,
    }
}

fn dark_pact_ability() -> AbilityDef {
    AbilityDef {
        name: "Dark Pact".to_string(),
        cooldown: 10.0,
        mana_cost: vec![50.0],
        cast_point: 0.0, // instant cast for testing
        targeting: TargetType::NoTarget,
        effects: vec![Effect::DarkPact {
            kind: DamageType::Magical,
            total_damage: vec![300.0],
            radius: vec![400.0],
            self_damage_pct: 0.3,
            delay: 1.5,
            pulse_count: 10,
            pulse_interval: 0.1,
            dispel_self: true,
            non_lethal: true,
        }],
        description: String::new(),
        aoe_shape: None,
        cast_range: 600.0,
    }
}

fn heavenly_grace_ability() -> AbilityDef {
    AbilityDef {
        name: "Heavenly Grace".to_string(),
        cooldown: 10.0,
        mana_cost: vec![50.0],
        cast_point: 0.0,
        targeting: TargetType::SingleAlly,
        effects: vec![Effect::BuffTargetAndSelf {
            name: "Heavenly Grace".to_string(),
            duration: vec![10.0],
            hp_regen: vec![20.0],
            strength: vec![30.0],
            status_resistance: vec![0.5],
        }],
        description: String::new(),
        aoe_shape: None,
        cast_range: 600.0,
    }
}

#[allow(dead_code)]
fn ravage_ability() -> AbilityDef {
    AbilityDef {
        name: "Ravage".to_string(),
        cooldown: 150.0,
        mana_cost: vec![150.0],
        cast_point: 0.0,
        targeting: TargetType::NoTarget,
        effects: vec![Effect::ExpandingWaveStun {
            damage: vec![250.0],
            stun_duration: vec![2.0],
            radius: vec![1025.0],
            wave_speed: 905.0,
        }],
        description: String::new(),
        aoe_shape: None,
        cast_range: 600.0,
    }
}

/// Helper: create a sim with a caster (team 0) and enemy (team 1) at given positions.
/// Caster has the given ability equipped. No AI (we manually trigger).
fn setup_dark_pact_sim(enemy_dist: f32) -> Simulation {
    let hero = make_hero();
    let config_a = UnitConfig::new(hero.clone()).with_ability(dark_pact_ability(), 1);
    let config_b = UnitConfig::new(hero);

    let mut u0 = Unit::from_config(&config_a, 0, 0, Vec2::new(0.0, 0.0));
    u0.mana = 500.0;
    let u1 = Unit::from_hero_def(&config_b.hero, 1, 1, Vec2::new(enemy_dist, 0.0));

    Simulation::new(vec![u0, u1])
}

#[test]
fn test_dark_pact_delay() {
    let hero = make_hero();
    // Place enemy far away so no auto-attacks happen
    let u0 = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    let u1 = Unit::from_hero_def(&hero, 1, 1, Vec2::new(5000.0, 0.0));

    let mut sim = Simulation::new(vec![u0, u1]);

    use aa2_sim::pending::{PendingEffect, PendingEffectKind};
    sim.pending_effects.push(PendingEffect {
        caster_id: 0,
        caster_team: 0,
        ability_name: "Dark Pact".to_string(),
        kind: PendingEffectKind::DarkPactPulse {
            damage_per_pulse: 30.0,
            radius: 10000.0, // large radius to hit distant enemy
            self_damage_pct: 0.3,
            damage_type: DamageType::Magical,
            dispel_self: true,
            non_lethal: true,
            pulses_remaining: 10,
            pulse_interval_ticks: 3,
            ticks_until_next_pulse: 0,
        },
        delay_ticks_remaining: 45,
    });

    // Run for 45 ticks (delay period) — no pulse events should occur
    for _ in 0..45 {
        sim.step();
    }

    assert!(
        !sim.combat_log.iter().any(|e| matches!(e, CombatEvent::DarkPactPulse { .. })),
        "No pulse events during delay"
    );

    // One more tick — first pulse should fire
    sim.step();
    assert!(
        sim.combat_log.iter().any(|e| matches!(e, CombatEvent::DarkPactPulse { .. })),
        "Pulse should fire after delay"
    );
}

#[test]
fn test_dark_pact_pulses() {
    let hero = make_hero();
    // Place enemy far away so no auto-attacks, but within pulse radius
    let u0 = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    let u1 = Unit::from_hero_def(&hero, 1, 1, Vec2::new(5000.0, 0.0));

    let mut sim = Simulation::new(vec![u0, u1]);

    use aa2_sim::pending::{PendingEffect, PendingEffectKind};
    sim.pending_effects.push(PendingEffect {
        caster_id: 0,
        caster_team: 0,
        ability_name: "Dark Pact".to_string(),
        kind: PendingEffectKind::DarkPactPulse {
            damage_per_pulse: 30.0,
            radius: 10000.0, // large radius to hit distant enemy
            self_damage_pct: 0.3,
            damage_type: DamageType::Magical,
            dispel_self: false,
            non_lethal: true,
            pulses_remaining: 10,
            pulse_interval_ticks: 3,
            ticks_until_next_pulse: 0,
        },
        delay_ticks_remaining: 0,
    });

    // Run enough ticks for all 10 pulses
    // Pulse fires when ticks_until_next_pulse == 0, then resets to 3
    // So pulses at ticks: 1, 4, 7, 10, 13, 16, 19, 22, 25, 28
    for _ in 0..40 {
        sim.step();
        if sim.is_finished() {
            break;
        }
    }

    let pulse_count = sim.combat_log.iter()
        .filter(|e| matches!(e, CombatEvent::DarkPactPulse { .. }))
        .count();
    assert_eq!(pulse_count, 10, "Should have exactly 10 pulses");

    // Verify pending effect is removed
    assert!(sim.pending_effects.is_empty(), "Pending effect should be removed after all pulses");
}

#[test]
fn test_dark_pact_self_damage() {
    let mut sim = setup_dark_pact_sim(100.0);

    use aa2_sim::pending::{PendingEffect, PendingEffectKind};
    sim.pending_effects.push(PendingEffect {
        caster_id: 0,
        caster_team: 0,
        ability_name: "Dark Pact".to_string(),
        kind: PendingEffectKind::DarkPactPulse {
            damage_per_pulse: 30.0,
            radius: 400.0,
            self_damage_pct: 0.3,
            damage_type: DamageType::Magical,
            dispel_self: false,
            non_lethal: true,
            pulses_remaining: 1,
            pulse_interval_ticks: 3,
            ticks_until_next_pulse: 0,
        },
        delay_ticks_remaining: 0,
    });

    let caster_hp_before = sim.units[0].hp;
    let caster_mr = sim.units[0].magic_resistance; // 0.25

    sim.step();

    // Self-damage: 30 * 0.3 = 9.0 raw, reduced by 25% MR = 6.75
    let expected_self_dmg = 9.0 * (1.0 - caster_mr);
    let actual_self_dmg = caster_hp_before - sim.units[0].hp;
    assert!(
        (actual_self_dmg - expected_self_dmg).abs() < 0.01,
        "Self-damage should be {expected_self_dmg}, got {actual_self_dmg}"
    );
}

#[test]
fn test_dark_pact_dispel() {
    let mut sim = setup_dark_pact_sim(100.0);

    // Apply a stun to caster (basic dispel level so strong dispel removes it)
    sim.units[0].buffs.push(Buff {
        name: "test_stun".to_string(),
        remaining_ticks: 300,
        tick_effect: None,
        stacking: StackBehavior::RefreshDuration,
        dispel_type: DispelType::BasicDispel,
        status: StatusFlags { stunned: true, ..StatusFlags::default() },
        stat_modifier: None,
        source_id: 1,
        is_debuff: true,
    });

    assert!(sim.units[0].buffs.iter().any(|b| b.name == "test_stun"));

    use aa2_sim::pending::{PendingEffect, PendingEffectKind};
    sim.pending_effects.push(PendingEffect {
        caster_id: 0,
        caster_team: 0,
        ability_name: "Dark Pact".to_string(),
        kind: PendingEffectKind::DarkPactPulse {
            damage_per_pulse: 30.0,
            radius: 400.0,
            self_damage_pct: 0.3,
            damage_type: DamageType::Magical,
            dispel_self: true,
            non_lethal: true,
            pulses_remaining: 1,
            pulse_interval_ticks: 3,
            ticks_until_next_pulse: 0,
        },
        delay_ticks_remaining: 0,
    });

    sim.step();

    // Stun should be dispelled
    assert!(
        !sim.units[0].buffs.iter().any(|b| b.name == "test_stun"),
        "Stun should be dispelled by Dark Pact pulse"
    );
}

#[test]
fn test_dark_pact_non_lethal() {
    let mut sim = setup_dark_pact_sim(5000.0); // enemy far away, won't be hit

    // Set caster to 1 HP
    sim.units[0].hp = 1.0;

    use aa2_sim::pending::{PendingEffect, PendingEffectKind};
    sim.pending_effects.push(PendingEffect {
        caster_id: 0,
        caster_team: 0,
        ability_name: "Dark Pact".to_string(),
        kind: PendingEffectKind::DarkPactPulse {
            damage_per_pulse: 100.0, // high damage
            radius: 400.0,
            self_damage_pct: 0.3,
            damage_type: DamageType::Magical,
            dispel_self: false,
            non_lethal: true,
            pulses_remaining: 10,
            pulse_interval_ticks: 3,
            ticks_until_next_pulse: 0,
        },
        delay_ticks_remaining: 0,
    });

    // Run all pulses
    for _ in 0..30 {
        sim.step();
    }

    // Caster should still be alive at 1 HP
    assert!(sim.units[0].hp >= 1.0, "Non-lethal self-damage should not kill caster");
    assert!(sim.units[0].is_alive(), "Caster should still be alive");
}

#[test]
fn test_expanding_wave() {
    let hero = make_hero();
    // Place enemies at different distances
    let u0 = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0)); // caster
    let u1 = Unit::from_hero_def(&hero, 1, 1, Vec2::new(100.0, 0.0)); // close
    let u2 = Unit::from_hero_def(&hero, 2, 1, Vec2::new(500.0, 0.0)); // far

    let mut sim = Simulation::new(vec![u0, u1, u2]);

    use aa2_sim::pending::{PendingEffect, PendingEffectKind};
    sim.pending_effects.push(PendingEffect {
        caster_id: 0,
        caster_team: 0,
        ability_name: "Ravage".to_string(),
        kind: PendingEffectKind::ExpandingWave {
            damage: 250.0,
            stun_duration_secs: 2.0,
            max_radius: 1025.0,
            wave_speed: 905.0,
            current_radius: 0.0,
            origin: Vec2::new(0.0, 0.0),
            already_hit: Vec::new(),
        },
        delay_ticks_remaining: 0,
    });

    // Wave speed 905 units/sec, tick = 1/30s, so ~30.17 units per tick
    // Unit at 100 should be hit around tick 4 (100/30.17 = 3.3)
    // Unit at 500 should be hit around tick 17 (500/30.17 = 16.6)

    let mut hit_ticks: Vec<(u32, u32)> = Vec::new(); // (unit_id, tick)
    for _ in 0..40 {
        sim.step();
        for event in &sim.combat_log {
            if let CombatEvent::WaveHit { target_id, tick, .. } = event
                && !hit_ticks.iter().any(|(id, _)| *id == *target_id)
            {
                hit_ticks.push((*target_id, *tick));
            }
        }
    }

    // Both should be hit
    assert!(hit_ticks.iter().any(|(id, _)| *id == 1), "Close enemy should be hit");
    assert!(hit_ticks.iter().any(|(id, _)| *id == 2), "Far enemy should be hit");

    // Close enemy should be hit before far enemy
    let close_tick = hit_ticks.iter().find(|(id, _)| *id == 1).unwrap().1;
    let far_tick = hit_ticks.iter().find(|(id, _)| *id == 2).unwrap().1;
    assert!(close_tick < far_tick, "Closer enemy should be stunned first: close={close_tick}, far={far_tick}");
}

#[test]
fn test_status_resistance() {
    let hero = make_hero();
    let mut u0 = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    u0.status_resistance = 0.5; // 50% status resistance

    // Apply a 2-second stun (60 ticks base)
    let base_ticks: u32 = 60;
    let actual_ticks = (base_ticks as f32 * (1.0 - u0.status_resistance)) as u32;
    assert_eq!(actual_ticks, 30, "50% status resistance should halve stun duration");

    // Verify via the expanding wave system
    let u1 = Unit::from_hero_def(&hero, 1, 1, Vec2::new(0.0, 0.0)); // caster at same pos
    let mut sim = Simulation::new(vec![u0, u1]);

    use aa2_sim::pending::{PendingEffect, PendingEffectKind};
    sim.pending_effects.push(PendingEffect {
        caster_id: 1,
        caster_team: 1,
        ability_name: "Ravage".to_string(),
        kind: PendingEffectKind::ExpandingWave {
            damage: 100.0,
            stun_duration_secs: 2.0,
            max_radius: 500.0,
            wave_speed: 905.0,
            current_radius: 0.0,
            origin: Vec2::new(0.0, 0.0),
            already_hit: Vec::new(),
        },
        delay_ticks_remaining: 0,
    });

    sim.step();

    // Check the stun buff on unit 0
    let stun = sim.units[0].buffs.iter().find(|b| b.name == "stun");
    assert!(stun.is_some(), "Unit should have stun buff");
    // With 50% status resistance, 2.0s (60 ticks) becomes 1.0s (30 ticks)
    // But one tick already passed in step_buffs, so it might be 29
    let remaining = stun.unwrap().remaining_ticks;
    assert!((28..=30).contains(&remaining),
        "Stun duration should be ~30 ticks (halved from 60), got {remaining}");
}

#[test]
fn test_buff_target_and_self() {
    let hero = make_hero();
    let config = UnitConfig::new(hero.clone()).with_ability(heavenly_grace_ability(), 1);

    let mut u0 = Unit::from_config(&config, 0, 0, Vec2::new(0.0, 0.0));
    u0.mana = 500.0;
    let u1 = Unit::from_hero_def(&hero, 1, 0, Vec2::new(100.0, 0.0)); // ally

    let mut sim = Simulation::new(vec![u0, u1]);

    // Manually execute the ability
    use aa2_sim::ability::execute_ability;
    let ability = heavenly_grace_ability();
    let events = execute_ability(
        &ability, 1, 0, 0, Vec2::new(0.0, 0.0),
        Some(1), Some(Vec2::new(100.0, 0.0)),
        &mut sim.units, 1, &mut sim.pending_effects,
    );

    // Both caster and target should have the buff
    let target_buff = sim.units[1].buffs.iter().find(|b| b.name == "Heavenly Grace");
    assert!(target_buff.is_some(), "Target should have Heavenly Grace buff");

    let caster_buff = sim.units[0].buffs.iter().find(|b| b.name == "Heavenly Grace");
    assert!(caster_buff.is_some(), "Caster should also have Heavenly Grace buff");

    // Verify stat modifier values
    let modifier = target_buff.unwrap().stat_modifier.as_ref().unwrap();
    assert!((modifier.bonus_hp_regen - 20.0).abs() < 0.01);
    assert!((modifier.bonus_strength - 30.0).abs() < 0.01);
    assert!((modifier.status_resistance - 0.5).abs() < 0.01);

    // Verify BuffApplied events for both
    let buff_events: Vec<_> = events.iter()
        .filter(|e| matches!(e, CombatEvent::BuffApplied { name, .. } if name == "Heavenly Grace"))
        .collect();
    assert_eq!(buff_events.len(), 2, "Should have BuffApplied for both target and caster");
}

#[test]
fn test_dark_pact_full_pipeline() {
    use std::path::Path;
    use aa2_sim::aa2_data::{load_loadout, resolve_loadout, UnitConfig};
    use aa2_sim::Simulation;

    let data_dir = Path::new("../../data");
    let loadout = load_loadout(Path::new("../../data/loadouts/jugg_darkpact.ron")).unwrap();
    let config = resolve_loadout(&loadout, data_dir).unwrap();

    // Verify ability loaded
    assert_eq!(config.abilities.len(), 1);
    assert_eq!(config.abilities[0].0.name, "Dark Pact");

    // Create sim with enemy nearby
    let hero2 = aa2_sim::aa2_data::load_hero_def(Path::new("../../data/heroes/sven.ron")).unwrap();
    let config_b = UnitConfig::new(hero2);

    let mut sim = Simulation::from_configs(&[config], &[config_b], 42);

    // Run for 3 ticks — cast should complete (0 cast point)
    for _ in 0..3 {
        sim.step();
    }

    // Verify pending effect was created
    println!("Pending effects after 3 ticks: {}", sim.pending_effects.len());
    assert!(!sim.pending_effects.is_empty(), "Dark Pact should create a pending effect");

    // Run until pulses fire (45 more ticks for delay + a few for pulses)
    for _ in 0..60 {
        sim.step();
    }

    // Check for DarkPactPulse events
    let pulse_events: Vec<_> = sim.combat_log.iter()
        .filter(|e| matches!(e, aa2_sim::CombatEvent::DarkPactPulse { .. }))
        .collect();
    println!("Pulse events: {}", pulse_events.len());
    assert!(!pulse_events.is_empty(), "Dark Pact pulses should have fired");
}
