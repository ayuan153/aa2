# AA2 Combat Mechanics Reference

> Implementation reference for the AA2 autobattler combat simulation.
> All formulas derived from Dota2 engine mechanics (Source 2, 30Hz server tick).

---

## Simulation Tick

```
TICK_RATE        = 30          // Hz
TICK_DURATION    = 1 / 30      // 0.03333s (33.33ms)
```

All game state advances once per tick. Continuous values (regen, movement) are applied as `value_per_second / 30` each tick.

---

## Attributes

| Attribute | Per-Point Bonus |
|-----------|----------------|
| STR | +22 max HP, +0.1 HP regen/sec |
| AGI | +1 attack speed, +0.167 armor |
| INT | +12 max mana, +0.05 mana regen/sec |
| Primary | +1 attack damage per point of primary attribute |

---

## Health & Mana

```
max_hp       = base_hp + str * 22
max_mana     = base_mana + int * 12
hp_regen     = base_hp_regen + str * 0.1        // per second
mana_regen   = base_mana_regen + int * 0.05     // per second

// Per-tick application:
hp_per_tick   = hp_regen / 30
mana_per_tick = mana_regen / 30
```

Typical base values: `base_hp = 120`, `base_mana = 75`, `base_hp_regen = 0.25`, `base_mana_regen = 0`.

---

## Attack System

```
total_attack_speed = clamp(100 + agi + bonus_as, 20, 700)
attack_interval    = BAT / (total_attack_speed / 100)

// Animation timing (seconds):
effective_attack_point = base_attack_point * (100 / total_attack_speed)
```

| Constant | Default | Notes |
|----------|---------|-------|
| BAT | 1.7s | Base Attack Time; varies per hero |
| AS min | 20 | Hard floor |
| AS max | 700 | Hard cap |

**Sequence:**
1. Unit begins attack → frontswing plays for `effective_attack_point` seconds.
2. On frontswing completion: melee deals damage instantly; ranged spawns projectile.
3. Backswing plays (cancellable — AA2 AI always cancels immediately).

**Edge cases:**
- At AS = 700: `attack_interval = 1.7 / 7.0 = 0.243s` (~7.3 ticks between attacks).
- At AS = 20: `attack_interval = 1.7 / 0.2 = 8.5s`.

---

## Projectiles

```
travel_time = distance / projectile_speed    // seconds
arrival_tick = current_tick + ceil(travel_time * 30)
```

- Homing: projectile tracks target position each tick. Cannot miss unless disjointed.
- Disjoint: if target becomes invalid (e.g., dies, banished), projectile is destroyed.
- Typical speeds: 900 (slow) to 3000 (fast) units/sec. Common attack projectile: 1100-1800.

---

## Armor & Physical Damage Reduction

```
// Works for positive AND negative armor:
damage_multiplier = 1 - (0.06 * armor) / (1 + 0.06 * abs(armor))

physical_damage_taken = raw_damage * damage_multiplier
```

| Armor | Multiplier | EHP factor |
|-------|-----------|------------|
| -10 | 1.375 | 0.727x |
| 0 | 1.000 | 1.000x |
| 10 | 0.625 | 1.600x |
| 20 | 0.455 | 2.200x |
| 30 | 0.357 | 2.800x |

```
armor_from_agi = agi * 0.167
```

**Edge case:** At armor = 0, multiplier = 1.0 (no reduction). Formula is continuous through zero.

---

## Damage Types

| Type | Reduced By |
|------|-----------|
| Physical | Armor |
| Magical | Magic Resistance |
| Pure | Nothing |

### Magic Resistance

```
base_magic_resistance = 0.25    // 25% for heroes

// Multiplicative stacking:
total_magic_resistance = 1 - (1 - base) * (1 - bonus1) * (1 - bonus2) * ...

magical_damage_taken = raw_damage * (1 - total_magic_resistance)
```

Example: 25% base + 30% bonus → `1 - (0.75 * 0.70) = 47.5%` resistance.

---

## Movement & Turn Rate

```
position += direction * move_speed * TICK_DURATION    // per tick

// Turn rate: radians turned per tick
radians_per_tick = turn_rate    // e.g., 0.6 rad/tick at 30Hz
time_to_turn = angle_remaining / turn_rate             // in ticks
```

| Constant | Value | Notes |
|----------|-------|-------|
| Typical turn rate | 0.5 – 0.9 rad/tick | Per hero |
| Action threshold | 11.5° (0.2007 rad) | Must face within this to attack/cast |
| Move speed range | 280 – 350 | Typical base |
| Move speed cap | 550 | Hard maximum |
| Collision radius | 24 units | Heroes |

**Sequence:** Unit must turn to face target (consuming ticks) before beginning attack/cast animation.

---

## Cast System

```
cast_point_ticks = ceil(cast_point_seconds * 30)
```

| Phase | Description |
|-------|-------------|
| Cast point | Animation before effect. Mana deducted here. Cooldown starts here. |
| Effect | Spell takes effect (damage, projectile spawn, buff applied). |
| Cast backswing | Post-cast animation. Cancellable, no gameplay impact. AA2 AI cancels. |

- **Channeling:** Effect applies continuously/periodically. Interrupted by: stun, silence, forced movement, hex.
- **Mana cost:** Deducted at cast point (not on channel start for channeled spells — at cast point).
- **Cooldown:** Begins counting down from the moment cast point completes.

---

## Targeting & Aggro

```
ACQUISITION_RANGE = 800    // units (default for heroes)
```

**AA2 target priority:** Closest enemy unit within acquisition range (Euclidean distance).

**Aggro rules:**
1. Unit attacks current target until target dies or leaves acquisition range.
2. If target lost, re-acquire using priority rules.
3. Forced target switch via taunt/aggro abilities overrides normal priority.

---

## Buffs & Debuffs

```
remaining_ticks = ceil(duration_seconds * 30)

// Tick-based damage (e.g., DoT):
damage_per_interval = total_damage / num_intervals
apply_every_n_ticks = ceil(interval_seconds * 30)
```

**Stacking rules:**
- Default: refresh duration (same source reapplies → timer resets).
- Intensity stacking: explicitly flagged per ability (multiple instances accumulate).

**Dispel types:** `BASIC_DISPEL`, `STRONG_DISPEL`, `UNDISPELLABLE`

**Status effects:**

| Status | Prevents |
|--------|----------|
| Stun | All actions (attack, cast, move, items) |
| Silence | Ability casting |
| Disarm | Attacking |
| Root | Movement (can still attack/cast) |
| Hex | All actions + sets MS to 140 + disables passives |

---

## Area of Effect

```
// Circle
hit = distance(unit.pos, center) <= radius

// Cone
hit = distance(unit.pos, origin) <= range
     AND angle_between(forward, unit.pos - origin) <= half_angle

// Line (rectangle)
hit = point_in_oriented_rect(unit.pos, origin, direction, width, length)
```

- **Ground-targeted:** AoE placed at a map position.
- **Unit-targeted:** AoE centered on/follows a unit.

---

## Death & Round Resolution (Autobattler)

```
if unit.hp <= 0:
    unit.state = DEAD
    remove from active_units

if count(team_a.active_units) == 0:
    winner = TEAM_B
elif count(team_b.active_units) == 0:
    winner = TEAM_A
```

- No respawn during combat rounds.
- Damage overkill is not tracked (HP floors at 0).
- Round ends immediately when one side is eliminated.
- Simultaneous kills on same tick: check both sides after all damage resolves.

---

## Tick Processing Order

Each tick processes in this order:
1. Expire/decrement buff/debuff timers
2. Apply tick-based effects (DoTs, regen)
3. Process movement (turn + translate)
4. Process attack/cast animations (advance timers)
5. Resolve hits (projectile arrivals, frontswing completions)
6. Apply damage and effects
7. Check deaths
8. Check round end condition
