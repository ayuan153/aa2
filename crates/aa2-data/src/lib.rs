use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Attribute {
    Strength,
    Agility,
    Intelligence,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum DamageType {
    Physical,
    Magical,
    Pure,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TargetType {
    SingleEnemy,
    SingleAlly,
    SingleAllyHG,
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
    /// Dark Pact style: delayed pulsing AoE around self with self-damage and per-pulse dispel.
    /// Caster can act freely during delay. Each pulse independently dispels.
    DarkPact {
        kind: DamageType,
        total_damage: Vec<f32>,
        radius: Vec<f32>,
        self_damage_pct: f32,
        delay: f32,
        pulse_count: u32,
        pulse_interval: f32,
        dispel_self: bool,
        non_lethal: bool,
    },
    /// Buff applied to target AND caster (Heavenly Grace self-cast mechanic).
    BuffTargetAndSelf {
        name: String,
        duration: Vec<f32>,
        hp_regen: Vec<f32>,
        strength: Vec<f32>,
        status_resistance: Vec<f32>,
        #[serde(default)]
        dispel_on_cast: bool,
    },
    /// Expanding wave AoE stun (Ravage). Hits units when wave reaches them.
    ExpandingWaveStun {
        damage: Vec<f32>,
        stun_duration: Vec<f32>,
        radius: Vec<f32>,
        wave_speed: f32,
    },
    /// Fury Swipes: per-target stacking flat damage, added post-crit.
    FurySwipes {
        damage_per_stack: Vec<f32>,
        stack_duration: Vec<f32>,
        armor_reduction_per_stack: Vec<f32>,
    },
    /// Chaos Strike: PRD-based crit with lifesteal.
    ChaosStrike {
        proc_chance: Vec<f32>,
        crit_min: Vec<f32>,
        crit_max: Vec<f32>,
        lifesteal: Vec<f32>,
    },
    /// Essence Shift: steal stats on attack.
    EssenceShift {
        str_steal: Vec<f32>,
        agi_steal: Vec<f32>,
        int_steal: Vec<f32>,
        agi_gain: Vec<f32>,
        duration: Vec<f32>,
    },
    /// Glaives of Wisdom: mana-cost attack modifier dealing bonus magical damage based on INT.
    /// Does not pierce debuff immunity. Super steals INT on kill. Gaben bounces.
    GlaivesOfWisdom {
        int_damage_factor: Vec<f32>,
        mana_cost: Vec<f32>,
        steal_int_on_kill: Vec<f32>,
        steal_radius: f32,
        bounce_radius: Vec<f32>,
    },
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
    /// Raw base damage range (before primary attribute bonus). [min, max]
    /// Each attack rolls uniformly between min and max (inclusive).
    pub base_damage_min: f32,
    pub base_damage_max: f32,
    /// Projectile speed for ranged heroes (units/sec). Ignored for melee.
    pub projectile_speed: Option<f32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AbilityDef {
    pub name: String,
    pub cooldown: Vec<f32>,
    pub mana_cost: Vec<f32>,
    pub cast_point: f32,
    pub targeting: TargetType,
    pub effects: Vec<Effect>,
    pub description: String,
    /// Shape of the AoE for `PointAoE` abilities. `None` for non-AoE abilities.
    #[serde(default)]
    pub aoe_shape: Option<AoeShape>,
    /// Maximum range at which this ability can be cast.
    #[serde(default = "default_cast_range")]
    pub cast_range: f32,
}

/// Default cast range for abilities (600 units).
fn default_cast_range() -> f32 {
    600.0
}

/// Look up a per-level value from a Vec (1-indexed: level 1 = index 0).
/// Clamps to the last element if level exceeds the array length.
pub fn value_at_level(values: &[f32], level: u8) -> f32 {
    let idx = (level.saturating_sub(1) as usize).min(values.len().saturating_sub(1));
    values[idx]
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

/// A fully-configured unit ready for combat.
/// Bridge between game systems (draft) and simulation.
#[derive(Debug, Clone)]
pub struct UnitConfig {
    /// The hero definition for this unit.
    pub hero: HeroDef,
    /// Equipped abilities with their levels.
    pub abilities: Vec<(AbilityDef, u8)>,
    /// Number of ability slots available.
    pub slot_count: u8,
    /// Hero level (1-30). Scales base attributes via gain per level.
    pub level: u8,
}

impl UnitConfig {
    /// Create a new UnitConfig with just a hero and no abilities.
    pub fn new(hero: HeroDef) -> Self {
        Self { hero, abilities: Vec::new(), slot_count: 4, level: 1 }
    }

    /// Add an ability at the given level.
    pub fn with_ability(mut self, ability: AbilityDef, level: u8) -> Self {
        self.abilities.push((ability, level));
        self
    }

    /// Set the hero level.
    pub fn with_level(mut self, level: u8) -> Self {
        self.level = level;
        self
    }
}

/// A loadout file specifying a hero + equipped abilities for dev/testing.
/// Ability and hero names are resolved to file paths at load time.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Loadout {
    /// Hero name (resolved to data/heroes/{name}.ron).
    pub hero: String,
    /// Ability names with levels (resolved to data/abilities/{name}.ron).
    pub abilities: Vec<(String, u8)>,
}

/// Loads a single `HeroDef` from a `.ron` file at the given path.
pub fn load_hero_def(path: &std::path::Path) -> Result<HeroDef, String> {
    let contents = std::fs::read_to_string(path).map_err(|e| format!("{path:?}: {e}"))?;
    ron::from_str(&contents).map_err(|e| format!("{path:?}: {e}"))
}

/// Loads a single `AbilityDef` from a `.ron` file at the given path.
pub fn load_ability_def(path: &std::path::Path) -> Result<AbilityDef, String> {
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

/// Load a `Loadout` from a `.ron` file.
pub fn load_loadout(path: &std::path::Path) -> Result<Loadout, String> {
    let contents = std::fs::read_to_string(path).map_err(|e| format!("{path:?}: {e}"))?;
    ron::from_str(&contents).map_err(|e| format!("{path:?}: {e}"))
}

/// Resolve a `Loadout` into a `UnitConfig` by loading hero and ability files from `data_dir`.
pub fn resolve_loadout(loadout: &Loadout, data_dir: &std::path::Path) -> Result<UnitConfig, String> {
    let hero_path = data_dir.join("heroes").join(format!("{}.ron", loadout.hero));
    let hero = load_hero_def(&hero_path)?;
    let mut config = UnitConfig::new(hero);
    for (ability_name, level) in &loadout.abilities {
        let ability_path = data_dir.join("abilities").join(format!("{}.ron", ability_name));
        let ability = load_ability_def(&ability_path)?;
        config.abilities.push((ability, *level));
    }
    Ok(config)
}
