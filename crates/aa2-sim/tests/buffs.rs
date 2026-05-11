//! Buff/debuff mechanics tests: stat modifiers, dispel interactions, and HP preservation.
//! This file will grow as more buff interaction tests are added.

use aa2_data::{Attribute, HeroDef};
use aa2_sim::buff::{Buff, DispelType, StackBehavior, StatModifier, StatusFlags};
use aa2_sim::unit::Unit;
use aa2_sim::vec2::Vec2;
use aa2_sim::Simulation;

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
