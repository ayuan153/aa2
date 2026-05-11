//! Integration tests encoding previously-manual verifications for key AA2 mechanics.
//! These tests use actual RON data files to prove both code AND data correctness.

use aa2_data::{load_ability_def, Attribute, HeroDef, UnitConfig};
use aa2_sim::buff::{Buff, DispelType, StackBehavior, StatModifier, StatusFlags};
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
    use aa2_data::{DamageType, Effect};
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

/// # Test: STR Buff Heals on Apply, Preserves HP on Expiry
///
/// Verifies Heavenly Grace's STR bonus interaction with HP:
/// 1. Gaining STR increases max_hp AND current hp (effective heal)
/// 2. Losing STR decreases max_hp but preserves current hp (capped at new max)
///
/// This matters because incorrect handling could either:
/// - Kill units when buff expires (if HP drops below 0)
/// - Give free permanent HP (if HP isn't capped on expiry)
#[test]
fn test_str_buff_heals_on_apply_preserves_on_expiry() {
    let hero = make_hero();
    let mut unit = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    let dummy = Unit::from_hero_def(&hero, 1, 1, Vec2::new(9999.0, 0.0));

    let base_max_hp = unit.max_hp; // 120 + 20*22 = 560

    // Damage unit to 400 HP
    unit.hp = 400.0;

    // Apply STR buff: +28 STR = +616 max_hp (28 * 22)
    unit.buffs.push(Buff {
        name: "Heavenly Grace".to_string(),
        remaining_ticks: 60, // 2 seconds — short for test
        tick_effect: None,
        stacking: StackBehavior::RefreshDuration,
        dispel_type: DispelType::BasicDispel,
        status: StatusFlags::default(),
        stat_modifier: Some(StatModifier { bonus_strength: 28.0, ..StatModifier::default() }),
        source_id: 0,
        is_debuff: false,
    });

    let mut sim = Simulation::new(vec![unit, dummy]);
    sim.step(); // Buff takes effect

    let expected_max = base_max_hp + 28.0 * 22.0; // 560 + 616 = 1176
    let expected_hp = 400.0 + 28.0 * 22.0; // 400 + 616 = 1016

    assert!(
        (sim.units[0].max_hp - expected_max).abs() < 2.0,
        "max_hp should be {expected_max}, got {}",
        sim.units[0].max_hp
    );
    assert!(
        (sim.units[0].hp - expected_hp).abs() < 2.0,
        "hp should be {expected_hp} (healed by STR gain), got {}",
        sim.units[0].hp
    );

    // --- Scenario A: HP above old max when buff expires ---
    // Set HP to 800 (above base_max_hp 560, below buffed max 1176)
    sim.units[0].hp = 800.0;

    // Run until buff expires (remaining ~59 ticks)
    for _ in 0..59 {
        sim.step();
    }

    // After expiry: max_hp returns to base, HP capped at new max
    assert!(
        (sim.units[0].max_hp - base_max_hp).abs() < 2.0,
        "max_hp should return to {base_max_hp}, got {}",
        sim.units[0].max_hp
    );
    // HP was 800 but max is now 560, so HP should be capped at 560 (plus tiny regen)
    assert!(
        sim.units[0].hp <= base_max_hp + 1.0,
        "HP should be capped at max_hp ({base_max_hp}), got {}",
        sim.units[0].hp
    );

    // --- Scenario B: HP below old max when buff expires ---
    let mut unit2 = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    let dummy2 = Unit::from_hero_def(&hero, 1, 1, Vec2::new(9999.0, 0.0));
    unit2.hp = 400.0;
    unit2.buffs.push(Buff {
        name: "Heavenly Grace".to_string(),
        remaining_ticks: 60,
        tick_effect: None,
        stacking: StackBehavior::RefreshDuration,
        dispel_type: DispelType::BasicDispel,
        status: StatusFlags::default(),
        stat_modifier: Some(StatModifier { bonus_strength: 28.0, ..StatModifier::default() }),
        source_id: 0,
        is_debuff: false,
    });

    let mut sim2 = Simulation::new(vec![unit2, dummy2]);
    sim2.step(); // Buff applies, HP goes to 1016

    // Set HP to 500 (below both old max 560 and new max 1176)
    sim2.units[0].hp = 500.0;

    for _ in 0..59 {
        sim2.step();
    }

    // After expiry: HP stays at 500 (below base max, so no capping needed)
    assert!(
        (sim2.units[0].max_hp - base_max_hp).abs() < 2.0,
        "max_hp should return to base"
    );
    // HP should be ~500 (plus tiny regen from the ticks)
    assert!(
        sim2.units[0].hp >= 500.0 && sim2.units[0].hp <= base_max_hp,
        "HP should be preserved at ~500, got {}",
        sim2.units[0].hp
    );
}
