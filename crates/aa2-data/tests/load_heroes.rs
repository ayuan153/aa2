use std::path::Path;

use aa2_data::{load_all_heroes, load_hero_def};

#[test]
fn load_warrior() {
    let hero = load_hero_def(Path::new("../../data/heroes/warrior.ron")).unwrap();
    assert_eq!(hero.name, "Warrior");
    assert_eq!(hero.base_str, 25.0);
    assert!(hero.is_melee);
}

#[test]
fn load_ranger() {
    let hero = load_hero_def(Path::new("../../data/heroes/ranger.ron")).unwrap();
    assert_eq!(hero.name, "Ranger");
    assert_eq!(hero.attack_range, 625.0);
    assert!(!hero.is_melee);
}

#[test]
fn load_all() {
    let heroes = load_all_heroes(Path::new("../../data/heroes")).unwrap();
    assert!(heroes.len() >= 2);
}
