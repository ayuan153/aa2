use std::path::Path;

use aa2_data::load_all_heroes;

#[test]
fn load_all() {
    let heroes = load_all_heroes(Path::new("../../data/heroes")).unwrap();
    assert!(heroes.len() >= 2);
}

#[test]
fn load_dark_pact() {
    let ability = aa2_data::load_ability_def(Path::new("../../data/abilities/dark_pact.ron")).unwrap();
    assert_eq!(ability.name, "Dark Pact");
    assert_eq!(ability.cast_point, 0.0);
    assert_eq!(ability.effects.len(), 1);
}

#[test]
fn load_fury_swipes() {
    let ability = aa2_data::load_ability_def(Path::new("../../data/abilities/fury_swipes.ron")).unwrap();
    assert_eq!(ability.name, "Fury Swipes");
}

#[test]
fn load_chaos_strike() {
    let ability = aa2_data::load_ability_def(Path::new("../../data/abilities/chaos_strike.ron")).unwrap();
    assert_eq!(ability.name, "Chaos Strike");
}

#[test]
fn load_essence_shift() {
    let ability = aa2_data::load_ability_def(Path::new("../../data/abilities/essence_shift.ron")).unwrap();
    assert_eq!(ability.name, "Essence Shift");
}

#[test]
fn load_glaives() {
    let ability = aa2_data::load_ability_def(Path::new("../../data/abilities/glaives_of_wisdom.ron")).unwrap();
    assert_eq!(ability.name, "Glaives of Wisdom");
}
