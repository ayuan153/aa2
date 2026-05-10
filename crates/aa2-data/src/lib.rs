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
