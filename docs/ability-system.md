# Ability Equip System Architecture

> How abilities get from "player drafted it" to "unit uses it in combat."

---

## Three-Layer Model

```
PlayerState (Phase 2: game systems)
    │
    │  .build_unit_configs()
    ▼
UnitConfig (Phase 1: bridge between game and sim)
    │
    │  Unit::from_config()
    ▼
Unit (Phase 0: combat simulation)
```

Each layer has a single responsibility and no knowledge of the layers above it.

---

## Layer 1: PlayerState (Phase 2)

Owns the player's draft state during a game. Lives in a future `aa2-game` crate.

```rust
pub struct PlayerState {
    pub god: GodDef,
    pub heroes: Vec<OwnedHero>,
    pub abilities: HashMap<AbilityId, OwnedAbility>,
    pub gold: u32,
    pub level: u8,
    pub hp: u32,
}

pub struct OwnedHero {
    pub def: HeroDef,
    pub ability_slots: Vec<Option<AbilityId>>,
    pub slot_count: u8,  // default 4, modified by god
}

pub struct OwnedAbility {
    pub def: AbilityDef,
    pub level: u8,                      // = copies purchased (1-9)
    pub assigned_to: Option<HeroIndex>, // which hero it's slotted on
}
```

### Rules enforced at this layer:
- **Uniqueness:** Each ability exists once per player (no duplicates)
- **Level = copies:** Buying 3 copies of Empower → level 3 Empower (not 3 separate level-1s)
- **Slot limits:** Cannot assign more abilities than `hero.slot_count`
- **Assignment exclusivity:** An ability can only be on one hero at a time
- **God modifiers:** Gods like Icefrog/Rubick increase slot_count beyond 4

### Produces:
`Vec<UnitConfig>` at combat time via `player.build_unit_configs()`.

---

## Layer 2: UnitConfig (Phase 1)

A fully-specified unit ready to enter combat. The bridge between game systems and simulation.

```rust
pub struct UnitConfig {
    pub hero: HeroDef,
    pub abilities: Vec<(AbilityDef, u8)>,  // (ability, level)
    pub slot_count: u8,                     // informational; sim doesn't enforce
}
```

### Properties:
- **Immutable snapshot:** Created once at combat start, never modified during combat
- **Self-contained:** Has all data needed to construct a Unit (no external lookups)
- **Sim-agnostic:** No combat state, no tick counters, no HP
- **Serializable:** Can be stored for replays, spectating, reconnect

### Used by:
- `Simulation::new()` accepts `Vec<UnitConfig>` per team
- Replay files store `Vec<UnitConfig>` as the initial state
- Spectator clients receive configs to render unit info

---

## Layer 3: Unit (Phase 0)

Runtime combat entity. Created from UnitConfig, mutated every tick.

```rust
impl Unit {
    pub fn from_config(config: &UnitConfig, id: u32, team: u8, position: Vec2) -> Self {
        let mut unit = Self::from_hero_def(&config.hero, id, team, position);
        for (ability_def, level) in &config.abilities {
            unit.abilities.push(AbilityState {
                def: ability_def.clone(),
                cooldown_remaining: 0.0,
                level: *level,
            });
        }
        unit
    }
}
```

### Properties:
- **Mutable:** HP, mana, position, buffs, cooldowns all change per tick
- **No game knowledge:** Doesn't know about gold, draft, or player state
- **Deterministic:** Given same UnitConfig + seed, produces identical combat

---

## Ability Slot Count

| Source | Slot Count | Notes |
|--------|-----------|-------|
| Default | 4 | Standard for all heroes |
| God: Icefrog | 6 | Limit break |
| God: Rubick | 5 | Extra slot for stolen ability |
| Items (future) | +1 | Aghanim's Scepter equivalent |

Slot count is set on `OwnedHero` by the god selection at game start. The sim receives the final equipped abilities and doesn't enforce slot limits (that's PlayerState's job).

---

## Ability Levels (1-9)

| Level Range | Tier | Notes |
|-------------|------|-------|
| 1-5 | Normal | Standard scaling |
| 6-8 | Super | Enhanced effects, visual upgrade |
| 9 | Gaben | Maximum power, unique visual |

Level is stored as a u8 index into the ability's `base` arrays (e.g., `Effect::Damage { base: [100, 150, 200, ...] }`). Level 1 = index 0.

Leveling happens by purchasing duplicate copies:
- Buy 1st copy → level 1
- Buy 2nd copy → level 2
- Buy 3rd copy → level 3
- ...up to 9

---

## Data Flow Example

```
1. Player picks god "Icefrog" (slot_count = 6)
2. Player drafts hero "Sven" → OwnedHero { def: sven, slots: [None; 6] }
3. Player buys "Fireball" → OwnedAbility { def: fireball, level: 1 }
4. Player buys "Fireball" again → level becomes 2
5. Player equips Fireball on Sven slot 0 → slots: [Some("fireball"), None, ...]
6. Player buys "War Cry" → OwnedAbility { def: war_cry, level: 1 }
7. Player equips War Cry on Sven slot 1

At combat time:
    player.build_unit_configs() → [
        UnitConfig {
            hero: sven_def,
            abilities: [(fireball_def, 2), (war_cry_def, 1)],
            slot_count: 6,
        }
    ]

Simulation receives this and creates:
    Unit {
        // sven stats...
        abilities: [
            AbilityState { def: fireball, level: 2, cooldown: 0 },
            AbilityState { def: war_cry, level: 1, cooldown: 0 },
        ],
    }
```

---

## Dev/Testing: Loadout Files

For Phase 1 testing without a full game loop, loadout files define pre-built units:

```ron
// data/loadouts/sven_nuker.ron
Loadout(
    hero: "sven",
    abilities: [
        ("fireball", 3),
        ("war_cry", 1),
    ],
)
```

The dev binary resolves string names to file paths:
- `"sven"` → `data/heroes/sven.ron`
- `"fireball"` → `data/abilities/fireball.ron`

Usage:
```bash
cargo run --bin aa2-dev -- --loadout data/loadouts/sven_nuker.ron data/loadouts/drow_support.ron
```

---

## Future Considerations

- **Ability synergies:** Some abilities interact (e.g., "if you have 3 fire abilities, gain +50% fire damage"). This is a modifier on UnitConfig, computed by PlayerState.
- **God passives:** Applied as permanent buffs at Unit creation time.
- **Items:** Added to UnitConfig, applied as stat modifiers during Unit::from_config().
- **Talent trees:** Per-ability upgrades that modify the AbilityDef before it reaches UnitConfig.
