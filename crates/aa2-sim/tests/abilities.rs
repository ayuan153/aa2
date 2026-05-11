//! Ability casting, execution, interactions, and scaling tests.
//! Covers Dark Pact, Heavenly Grace, Ravage, and their interactions.

use aa2_data::{AbilityDef, Attribute, DamageType, Effect, HeroDef, TargetType, UnitConfig};
use aa2_sim::buff::{Buff, DispelType, StackBehavior, StatusFlags};
use aa2_sim::unit::Unit;
use aa2_sim::vec2::Vec2;
use aa2_sim::{CombatEvent, Simulation};
use std::path::Path;

fn data_path(relative: &str) -> std::path::PathBuf {
    Path::new("../../data").join(relative)
}

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
        cast_point: 0.0,
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
            dispel_on_cast: false,
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
fn test_hg_dispels_on_cast() {
    use aa2_sim::ability::execute_ability;

    let hero = make_hero();
    let mut u0 = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    u0.mana = 500.0;
    let mut u1 = Unit::from_hero_def(&hero, 1, 0, Vec2::new(100.0, 0.0)); // ally

    // Apply a debuff to the ally
    u1.buffs.push(Buff {
        name: "curse".to_string(),
        remaining_ticks: 300,
        tick_effect: None,
        stacking: aa2_sim::buff::StackBehavior::RefreshDuration,
        dispel_type: DispelType::StrongDispel,
        status: StatusFlags { silenced: true, ..StatusFlags::default() },
        stat_modifier: None,
        source_id: 99,
        is_debuff: true,
    });

    let mut units = vec![u0, u1];
    let ability = AbilityDef {
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
            dispel_on_cast: true,
        }],
        description: String::new(),
        aoe_shape: None,
        cast_range: 600.0,
    };

    execute_ability(
        &ability, 1, 0, 0, Vec2::new(0.0, 0.0),
        Some(1), Some(Vec2::new(100.0, 0.0)),
        &mut units, 1, &mut Vec::new(),
    );

    // Debuff should be removed from ally
    assert!(
        !units[1].buffs.iter().any(|b| b.name == "curse"),
        "Strong dispel should remove the curse debuff"
    );
    // HG buff should be applied
    assert!(units[1].buffs.iter().any(|b| b.name == "Heavenly Grace"));
}

#[test]
fn test_hg_targets_highest_y_ally() {
    use aa2_sim::ai::try_find_cast;
    use aa2_sim::cast::AbilityState;

    let hero = make_hero();
    let mut u0 = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    u0.mana = 500.0;
    u0.abilities.push(AbilityState {
        def: AbilityDef {
            name: "Heavenly Grace".to_string(),
            cooldown: 10.0,
            mana_cost: vec![50.0],
            cast_point: 0.0,
            targeting: TargetType::SingleAllyHG,
            effects: vec![],
            description: String::new(),
            aoe_shape: None,
            cast_range: 600.0,
        },
        cooldown_remaining: 0.0,
        level: 1,
        casts: 0, // first cast
    });

    // Allies at different y positions
    let u1 = Unit::from_hero_def(&hero, 1, 0, Vec2::new(50.0, 100.0));  // y=100
    let u2 = Unit::from_hero_def(&hero, 2, 0, Vec2::new(50.0, 300.0));  // y=300 (highest)
    let u3 = Unit::from_hero_def(&hero, 3, 0, Vec2::new(50.0, 200.0));  // y=200

    let units = vec![u0, u1, u2, u3];
    let result = try_find_cast(&units[0], &units);

    assert!(result.is_some());
    let (_, target_id, _) = result.unwrap();
    assert_eq!(target_id, Some(2), "Should target ally with highest y (id=2, y=300)");
}

#[test]
fn test_hg_self_cast_when_no_allies() {
    use aa2_sim::ai::try_find_cast;
    use aa2_sim::cast::AbilityState;

    let hero = make_hero();
    let mut u0 = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    u0.mana = 500.0;
    u0.abilities.push(AbilityState {
        def: AbilityDef {
            name: "Heavenly Grace".to_string(),
            cooldown: 10.0,
            mana_cost: vec![50.0],
            cast_point: 0.0,
            targeting: TargetType::SingleAllyHG,
            effects: vec![],
            description: String::new(),
            aoe_shape: None,
            cast_range: 600.0,
        },
        cooldown_remaining: 0.0,
        level: 1,
        casts: 0,
    });

    // Only enemies, no allies
    let u1 = Unit::from_hero_def(&hero, 1, 1, Vec2::new(100.0, 0.0));

    let units = vec![u0, u1];
    let result = try_find_cast(&units[0], &units);

    assert!(result.is_some());
    let (_, target_id, _) = result.unwrap();
    assert_eq!(target_id, Some(0), "Should self-cast when no allies in range");
}

#[test]
fn test_hg_targets_furthest_on_subsequent_cast() {
    use aa2_sim::ai::try_find_cast;
    use aa2_sim::cast::AbilityState;

    let hero = make_hero();
    let mut u0 = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    u0.mana = 500.0;
    u0.abilities.push(AbilityState {
        def: AbilityDef {
            name: "Heavenly Grace".to_string(),
            cooldown: 10.0,
            mana_cost: vec![50.0],
            cast_point: 0.0,
            targeting: TargetType::SingleAllyHG,
            effects: vec![],
            description: String::new(),
            aoe_shape: None,
            cast_range: 600.0,
        },
        cooldown_remaining: 0.0,
        level: 1,
        casts: 1, // subsequent cast
    });

    // Ally at y=300 but close (50 units away)
    let u1 = Unit::from_hero_def(&hero, 1, 0, Vec2::new(50.0, 300.0));
    // Ally at y=100 but far (500 units away)
    let u2 = Unit::from_hero_def(&hero, 2, 0, Vec2::new(400.0, 300.0));

    let units = vec![u0, u1, u2];
    let result = try_find_cast(&units[0], &units);

    assert!(result.is_some());
    let (_, target_id, _) = result.unwrap();
    assert_eq!(target_id, Some(2), "Should target furthest ally on subsequent cast");
}

/// # Test: AoE Radius Scaling — Gaben (level 9) vs Level 3
///
/// Verifies that Dark Pact's radius scales correctly with level:
/// - Level 9 (Gaben): radius 675 → hits enemy at 600 distance
/// - Level 3: radius 325 → does NOT hit enemy at 600 distance
///
/// This matters because radius scaling is the primary power curve for Dark Pact,
/// and incorrect radius values would make the ability over/under-powered.
#[test]
fn test_aoe_radius_scaling_gaben_vs_level3() {
    use aa2_data::load_ability_def;

    let dark_pact = load_ability_def(&data_path("abilities/dark_pact.ron")).unwrap();
    let hero = make_hero();

    // Extract radius values from the loaded ability data to verify correctness
    let effect = &dark_pact.effects[0];
    let (radius_l9, radius_l3, total_dmg_l9, total_dmg_l3) = match effect {
        Effect::DarkPact { radius, total_damage, pulse_count, .. } => {
            (
                aa2_data::value_at_level(radius, 9),
                aa2_data::value_at_level(radius, 3),
                aa2_data::value_at_level(total_damage, 9) / *pulse_count as f32,
                aa2_data::value_at_level(total_damage, 3) / *pulse_count as f32,
            )
        }
        _ => panic!("Expected DarkPact effect"),
    };
    assert_eq!(radius_l9, 675.0);
    assert_eq!(radius_l3, 325.0);

    // Directly inject pending effects to test radius without caster movement.
    // Caster at origin, enemy at exactly 600 units away.
    let caster = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    let enemy = Unit::from_hero_def(&hero, 1, 1, Vec2::new(600.0, 0.0));

    // --- Gaben (radius 675): should hit at 600 ---
    let mut sim_gaben = Simulation::new(vec![caster.clone(), enemy.clone()]);
    sim_gaben.pending_effects.push(aa2_sim::pending::PendingEffect {
        caster_id: 0,
        caster_team: 0,
        ability_name: "Dark Pact".to_string(),
        kind: aa2_sim::pending::PendingEffectKind::DarkPactPulse {
            damage_per_pulse: total_dmg_l9,
            radius: radius_l9,
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
    sim_gaben.step();

    // --- Level 3 (radius 325): should NOT hit at 600 ---
    let mut sim_l3 = Simulation::new(vec![caster, enemy]);
    sim_l3.pending_effects.push(aa2_sim::pending::PendingEffect {
        caster_id: 0,
        caster_team: 0,
        ability_name: "Dark Pact".to_string(),
        kind: aa2_sim::pending::PendingEffectKind::DarkPactPulse {
            damage_per_pulse: total_dmg_l3,
            radius: radius_l3,
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
    sim_l3.step();

    let gaben_hit = sim_gaben.combat_log.iter().find_map(|e| {
        if let CombatEvent::DarkPactPulse { enemies_hit, .. } = e { Some(*enemies_hit) } else { None }
    });
    let l3_hit = sim_l3.combat_log.iter().find_map(|e| {
        if let CombatEvent::DarkPactPulse { enemies_hit, .. } = e { Some(*enemies_hit) } else { None }
    });

    assert_eq!(gaben_hit, Some(1), "Gaben Dark Pact (radius 675) should hit enemy at 600 distance");
    assert_eq!(l3_hit, Some(0), "Level 3 Dark Pact (radius 325) should NOT hit enemy at 600 distance");
}

/// # Test: Ravage Wave Timing — Distance-Based Stun Order
///
/// Verifies that Ravage's expanding wave stuns closer enemies before farther ones.
/// This is the core mechanic that differentiates Ravage from instant AoE stuns:
/// positioning matters because farther enemies have time to react.
///
/// Wave speed: 905 units/sec = ~30.17 units/tick.
/// Expected tick difference for 300 units: ~10 ticks.
#[test]
fn test_ravage_wave_timing_distance_based() {
    use aa2_data::load_ability_def;

    let ravage = load_ability_def(&data_path("abilities/ravage.ron")).unwrap();
    let hero = make_hero();

    // Verify loaded data
    let effect = &ravage.effects[0];
    let (damage, stun_dur, wave_speed) = match effect {
        aa2_data::Effect::ExpandingWaveStun { damage, stun_duration, wave_speed, .. } => {
            (aa2_data::value_at_level(damage, 2), aa2_data::value_at_level(stun_duration, 2), *wave_speed)
        }
        _ => panic!("Expected ExpandingWaveStun"),
    };
    assert_eq!(wave_speed, 905.0);
    assert_eq!(stun_dur, 2.2);

    // Inject wave directly at origin to avoid cast point and enemy movement.
    // Place caster far away (outside acquisition range 800) so enemies don't walk.
    // Wave origin is set to (0,0) independently of caster position.
    let caster = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, -1000.0));
    let enemy_a = Unit::from_hero_def(&hero, 1, 1, Vec2::new(200.0, 0.0));
    let enemy_b = Unit::from_hero_def(&hero, 2, 1, Vec2::new(500.0, 0.0));

    let mut sim = Simulation::new(vec![caster, enemy_a, enemy_b]);
    sim.pending_effects.push(aa2_sim::pending::PendingEffect {
        caster_id: 0,
        caster_team: 0,
        ability_name: "Ravage".to_string(),
        kind: aa2_sim::pending::PendingEffectKind::ExpandingWave {
            damage,
            stun_duration_secs: stun_dur,
            max_radius: 700.0,
            wave_speed,
            current_radius: 0.0,
            origin: Vec2::new(0.0, 0.0),
            already_hit: Vec::new(),
        },
        delay_ticks_remaining: 0,
    });

    // Run enough ticks for wave to reach 500 units: 500/30.17 ≈ 17 ticks
    for _ in 0..25 {
        sim.step();
    }

    let hit_a_tick = sim.combat_log.iter().find_map(|e| {
        if let CombatEvent::WaveHit { target_id: 1, tick, .. } = e { Some(*tick) } else { None }
    });
    let hit_b_tick = sim.combat_log.iter().find_map(|e| {
        if let CombatEvent::WaveHit { target_id: 2, tick, .. } = e { Some(*tick) } else { None }
    });

    let tick_a = hit_a_tick.expect("Enemy A (200 units) should be hit by Ravage");
    let tick_b = hit_b_tick.expect("Enemy B (500 units) should be hit by Ravage");

    assert!(tick_a < tick_b, "Closer enemy should be stunned first: A@tick {tick_a}, B@tick {tick_b}");

    // Expected difference: (500-200) / (905/30) = 300 / 30.17 ≈ 9.9 ticks
    let diff = tick_b - tick_a;
    assert!(
        (8..=12).contains(&diff),
        "Tick difference should be ~10 (got {diff}): 300 units / (905/30) units_per_tick"
    );
}

/// # Test: Dark Pact Dispels Ravage Stun
///
/// Verifies the key interaction: Dark Pact's self-dispel removes Ravage's stun early.
/// This is the primary reason to pick Dark Pact — it counters hard disables.
///
/// Timeline:
/// - Tick 1: Unit A casts Dark Pact (instant, 1.5s delay before pulses)
/// - Tick ~10: Ravage wave hits Unit A, applying 2.2s stun (66 ticks)
/// - Tick ~46: Dark Pact pulses begin, dispelling the stun
/// - Without dispel, stun would last until tick ~76
#[test]
fn test_dark_pact_dispels_ravage_stun() {
    use aa2_data::load_ability_def;

    let dark_pact = load_ability_def(&data_path("abilities/dark_pact.ron")).unwrap();
    let ravage = load_ability_def(&data_path("abilities/ravage.ron")).unwrap();
    let hero = make_hero();

    // Unit A: has Dark Pact (team 0), facing Unit B
    let config_a = UnitConfig::new(hero.clone()).with_ability(dark_pact, 3);
    let mut unit_a = Unit::from_config(&config_a, 0, 0, Vec2::new(0.0, 0.0));
    unit_a.mana = 500.0;
    unit_a.facing = std::f32::consts::PI; // facing toward Unit B at negative X... actually let's place B at +X
    unit_a.facing = 0.0; // facing +X

    // Unit B: has Ravage (team 1), facing Unit A
    let config_b = UnitConfig::new(hero.clone()).with_ability(ravage, 2);
    let mut unit_b = Unit::from_config(&config_b, 1, 1, Vec2::new(400.0, 0.0));
    unit_b.mana = 500.0;
    unit_b.facing = std::f32::consts::PI; // facing -X toward Unit A

    let mut sim = Simulation::new(vec![unit_a, unit_b]);

    // Run simulation until Unit A attacks (proving stun was dispelled)
    let mut first_attack_tick: Option<u32> = None;
    let mut stun_applied_tick: Option<u32> = None;

    for _ in 0..120 {
        sim.step();
        if stun_applied_tick.is_none()
            && let Some(CombatEvent::WaveHit { target_id: 0, tick, .. }) =
                sim.combat_log.iter().find(|e| matches!(e, CombatEvent::WaveHit { target_id: 0, .. }))
        {
            stun_applied_tick = Some(*tick);
        }
        if first_attack_tick.is_none()
            && let Some(CombatEvent::Attack { attacker_id: 0, tick, .. }) =
                sim.combat_log.iter().find(|e| matches!(e, CombatEvent::Attack { attacker_id: 0, .. }))
        {
            first_attack_tick = Some(*tick);
            break;
        }
    }

    let stun_tick = stun_applied_tick.expect("Ravage should stun Unit A");
    let attack_tick = first_attack_tick.expect("Unit A should attack after dispel");

    // Ravage level 2 stun = 2.2s = 66 ticks. Without dispel, stun expires at stun_tick + 66.
    let stun_natural_expiry = stun_tick + 66;

    assert!(
        attack_tick < stun_natural_expiry,
        "Dark Pact should dispel stun early: attack@{attack_tick} < natural_expiry@{stun_natural_expiry}"
    );
}
