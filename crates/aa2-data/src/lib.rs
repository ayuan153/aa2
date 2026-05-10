use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Attribute {
    Strength,
    Agility,
    Intelligence,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DamageType {
    Physical,
    Magical,
    Pure,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TargetType {
    SingleEnemy,
    SingleAlly,
    PointAoE,
    NoTarget,
    Passive,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AoeShape {
    Circle { radius: f32 },
    Cone { angle: f32, range: f32 },
    Line { width: f32, length: f32 },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Effect {
    Damage { kind: DamageType, base: Vec<f32> },
    ApplyBuff { name: String, duration: f32 },
    Heal { base: Vec<f32> },
    Summon { unit: String, count: u32 },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HeroDef {
    pub name: String,
    pub primary_attribute: Attribute,
    pub base_str: f32,
    pub base_agi: f32,
    pub base_int: f32,
    pub str_gain: f32,
    pub agi_gain: f32,
    pub int_gain: f32,
    pub base_attack_time: f32,
    pub attack_range: f32,
    pub attack_point: f32,
    pub move_speed: f32,
    pub turn_rate: f32,
    pub collision_radius: f32,
    pub tier: u8,
    pub is_melee: bool,
    /// Raw base damage (before primary attribute bonus). Average of min/max.
    pub base_damage: f32,
    /// Projectile speed for ranged heroes (units/sec). Ignored for melee.
    pub projectile_speed: Option<f32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AbilityDef {
    pub name: String,
    pub cooldown: f32,
    pub mana_cost: f32,
    pub cast_point: f32,
    pub targeting: TargetType,
    pub effects: Vec<Effect>,
    pub description: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GodDef {
    pub name: String,
    pub passive_description: String,
    pub active_description: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StatBonuses {
    pub strength: f32,
    pub agility: f32,
    pub intelligence: f32,
    pub attack_speed: f32,
    pub move_speed: f32,
    pub armor: f32,
    pub magic_resistance: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ItemDef {
    pub name: String,
    pub tier: u8,
    pub effects: Vec<Effect>,
    pub stat_bonuses: StatBonuses,
}

/// Loads a single `HeroDef` from a `.ron` file at the given path.
pub fn load_hero_def(path: &std::path::Path) -> Result<HeroDef, String> {
    let contents = std::fs::read_to_string(path).map_err(|e| format!("{path:?}: {e}"))?;
    ron::from_str(&contents).map_err(|e| format!("{path:?}: {e}"))
}

/// Loads all `HeroDef`s from `.ron` files in the given directory.
pub fn load_all_heroes(dir: &std::path::Path) -> Result<Vec<HeroDef>, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{dir:?}: {e}"))?;
    let mut heroes = Vec::new();
    for entry in entries {
        let path = entry.map_err(|e| format!("{dir:?}: {e}"))?.path();
        if path.extension().is_some_and(|ext| ext == "ron") {
            heroes.push(load_hero_def(&path)?);
        }
    }
    Ok(heroes)
}
