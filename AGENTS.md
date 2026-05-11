# AGENTS.md — AI Agent Guidelines for AA2

## Commit Convention

Use Conventional Commits: `type(scope): description`

Types: `feat:` | `fix:` | `refactor:` | `docs:` | `test:` | `chore:`

Examples:
- `feat(sim): implement attack speed calculation`
- `fix(data): correct armor formula sign handling`
- `test(sim): add projectile travel time tests`

Commit messages MUST include a `Prompt:` trailer line describing what was asked:
```
feat(sim): implement buff stacking system

Implement multiplicative and additive buff stacking with
duration refresh and independent stack tracking.

Prompt: implement the buff/debuff framework with stack rules
```

## Test Loop (MANDATORY)

Before any commit:
1. `cargo check` — must pass with no errors
2. `cargo test` — all tests must pass
3. `cargo clippy` — no warnings (treat warnings as errors)

When implementing a new mechanic:
1. Write a failing test first (reference `docs/mechanics-reference.md` for expected values)
2. Implement until test passes
3. Run full test suite to ensure no regressions

## Integration Tests (MANDATORY)

**Every manual verification MUST become an automated test.** If you ran the dev binary and checked the output to verify something works, that verification must be encoded as a test before the work is considered done.

### When to write an integration test

- You verified a mechanic by reading combat log output → write a test
- You confirmed an interaction between two abilities → write a test
- You checked that a value scales correctly at different levels → write a test
- You validated timing (ticks, delays, travel time) → write a test

### Pattern

```rust
/// [What this tests] — [Why it matters for game feel]
#[test]
fn test_specific_interaction() {
    // 1. Load actual RON data files (proves data + code work together)
    let ability = load_ability_def(Path::new("../../data/abilities/X.ron")).unwrap();

    // 2. Set up exact initial conditions (deterministic)
    let mut unit = Unit::from_hero_def(&hero, 0, 0, Vec2::new(0.0, 0.0));
    unit.abilities.push(AbilityState { def: ability, level: 3, .. });

    // 3. Run simulation with fixed seed
    let mut sim = Simulation::with_seed(vec![unit, enemy], 42);
    for _ in 0..N { sim.step(); }

    // 4. Assert specific properties (not approximate unless necessary)
    assert!(sim.combat_log.iter().any(|e| matches!(e, CombatEvent::X { .. })));
}
```

### Where to put tests

| Test type | Location | Tests what |
|-----------|----------|-----------|
| Unit tests | `src/*.rs` (`#[cfg(test)]` modules) | Individual formulas, single-function behavior |
| Integration tests | `tests/*.rs` | Multi-system interactions, full sim runs, data file loading |

### Rules

- Tests MUST be deterministic (fixed seed, exact positions, no timing dependencies)
- Tests MUST be fast (< 100ms each, ideally < 1ms)
- Tests MUST use actual RON data files where the mechanic depends on data values
- Tests MUST have a doc comment explaining what interaction they verify
- Never verify by "eyeballing" output — if it's worth checking, it's worth asserting

## Documentation Updates

When making changes that affect architecture or project plan:
- Update `docs/architecture.md` if system design changes
- Update `docs/project-plan.md` if milestones shift
- Update `docs/mechanics-reference.md` if formula implementations reveal corrections
- Add inline doc comments (`///`) to all public types and functions

## Code Style

- Follow standard Rust idioms (clippy is the authority)
- All public items must have doc comments
- Use `#[must_use]` on functions that return values that shouldn't be ignored
- Prefer `f32` for game math (server-authoritative, no determinism requirement on client)
- Use descriptive variable names matching the mechanics reference (e.g., `base_attack_time` not `bat`)

## Data Files

- Game data lives in `data/` as RON files
- RON files must include comments explaining non-obvious values
- All data types live in `aa2-data` crate
- Test deserialization of sample data files in integration tests

## Architecture Rules

- `aa2-data`: ONLY data types and deserialization. No game logic.
- `aa2-sim`: Combat simulation. Depends on aa2-data. No I/O, no networking, no rendering.
- `aa2-server`: Networking + game flow. Depends on aa2-sim. (Phase 3+)
- Keep crates independent — sim must compile to WASM and native iOS without modification.

## Working on This Project

1. Read `docs/mechanics-reference.md` before implementing any combat mechanic
2. Read `docs/architecture.md` before adding new systems
3. Check `docs/project-plan.md` to understand current phase and priorities
4. When in doubt about a Dota2 mechanic, cite the source (wiki/liquipedia)

## Priorities (in order)

1. Correctness (mechanics must match Dota2 formulas exactly)
2. Testability (every formula must have a unit test)
3. Performance (30Hz tick with 50+ units must be <5ms per tick)
4. Readability (code should be self-documenting with doc comments)
