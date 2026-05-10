# AA2 — Phased Development Plan

Solo-dev project (with AI agent assistance). Cross-platform autobattler with Dota2-fidelity combat simulation.

**Timeline:** ~36 weeks to platform release, ongoing content thereafter.

---

## Phase 0: Foundation + Dev Mode (Weeks 1–4)

| Week | Focus |
|------|-------|
| 1 | Monorepo setup (Rust workspace + Unity project), FFI bridge prototype |
| 2 | aa2-sim crate: ECS skeleton, attribute system |
| 3 | Basic attack loop (BAT, attack speed, armor reduction) |
| 4 | Unity combat viewer (1v1, placeholder art, dev mode) |

**Deliverables:**
- Rust workspace with `aa2-sim` crate
- Unity native plugin loading Rust dylib via C FFI
- LOCAL DEV MODE: sim runs in-process, 1v1 combat viewer
- Placeholder art (colored polygons with labels)

**Milestone:** Two units fighting with correct Dota2 attack timing.

**Success Criteria:**
- Attack interval matches `BAT / (AS / 100)` formula
- Damage reduced by armor formula: `multiplier = 1 - (0.06 * armor) / (1 + 0.06 * |armor|)`
- FFI bridge works on macOS and iOS simulator

---

## Phase 1: Combat Fidelity (Weeks 5–12)

| Week | Focus |
|------|-------|
| 5–6 | Full attribute system (STR/AGI/INT → derived stats) |
| 7 | Turn rate, cast points, attack animations (frontswing/backswing) |
| 8 | Projectile system (homing, speed-based travel time) |
| 9 | Buff/debuff framework (stacking, duration, tick effects, dispel types) |
| 10 | AoE system (circle, cone, line), damage types (physical/magical/pure) |
| 11 | Targeting AI (acquisition range, aggro, priority), grid pathfinding |
| 12 | DEV MODE: 5v5 bot draft, 8-board view, hot-reload data, replay system |

**Deliverables:**
- Complete combat simulation matching Dota2 mechanics
- Replay recording + deterministic playback
- Dev mode with 5v5 bot battles and data hot-reload

**Milestone:** 5v5 combat that feels like Dota2.

**Success Criteria:**
- Side-by-side comparison with Dota2 mod shows matching timing/behavior
- Projectile travel time, turn rates, and cast points within 1 tick of Dota2 values
- Replays are deterministic (same seed → identical outcome)

---

## Phase 2: Game Systems (Weeks 13–20)

| Week | Focus |
|------|-------|
| 13–14 | God selection + god abilities (8–10 gods for MVP) |
| 15–16 | Draft/shop system (ability pool, reroll, level up, interest gold) |
| 17 | Hero body system (tiers 1–4, base stats from data files) |
| 18 | Ability equip system (slot onto heroes, level 1–9 scaling) |
| 19 | Round structure (PvP matchups, PvE rounds, elimination) |
| 20 | AI opponents (random → heuristic draft), dev mode: control all 8 slots |

**Deliverables:**
- Full game loop: god pick → draft → combat → elimination → placement
- AI opponents with basic drafting heuristics
- Dev mode: developer controls all 8 player slots

**Milestone:** Complete game loop playable locally.

**Success Criteria:**
- Can play a full game from god pick to final placement
- Economy math works (gold income, interest, reroll cost)
- AI opponents make non-random draft decisions

---

## Phase 3: Multiplayer (Weeks 21–28)

| Week | Focus |
|------|-------|
| 21–22 | aa2-server binary (headless sim, WebSocket server) |
| 23–24 | State-sync protocol (10Hz snapshots, delta compression) |
| 25 | Matchmaking + lobby system (region + MMR filtering) |
| 26 | Reconnect support (full state snapshot on rejoin) |
| 27 | Spectating (subscribe to other player boards) |
| 28 | Anti-cheat (server-authoritative validation), load testing |

**Deliverables:**
- Dedicated server binary running headless simulation
- WebSocket-based state sync with delta compression
- Matchmaking, reconnect, and spectating

**Milestone:** 8 humans playing online.

**Success Criteria:**
- Stable 8-player game with <100ms perceived latency
- Reconnect restores full game state within 2 seconds
- Server validates all client actions (no trust-the-client)

---

## Phase 4: Polish + Platform (Weeks 29–36)

| Week | Focus |
|------|-------|
| 29–30 | Full UI/UX (draft screen, shop, combat viewer, scoreboard) |
| 31–32 | Art assets (AI-generated chibi characters, ability VFX, audio) |
| 33 | iOS build + TestFlight submission |
| 34 | Android build + Play Store |
| 35 | Steam integration (achievements, friends) |
| 36 | F2P monetization (battle pass, cosmetics shop, IAP) |

**Deliverables:**
- Production UI across all game screens
- Art and audio assets (AI-generated where possible)
- Builds for iOS, Android, and Steam

**Milestone:** App Store approved, playable on all platforms.

**Success Criteria:**
- Passes Apple review on first or second submission
- Runs at 60fps on iPhone 12+
- IAP and battle pass functional on all platforms

---

## Phase 5: Content + Launch (Weeks 37+)

- Expand to full god roster, ability pool, hero bodies
- Balance tuning via automated simulation + manual adjustment
- Launch cadence: closed beta → open beta → soft launch → full launch
- Seasonal content: new gods, abilities, battle pass each season
- Ongoing: community feedback, balance patches, live ops

**Milestone:** Sustainable live game with active player base.

**Success Criteria:**
- Day-7 retention > 20%
- Stable matchmaking queue times < 60s at launch

---

## Risk Register

| Risk | Impact | Mitigation |
|------|--------|------------|
| iOS App Store rejection | Blocks mobile launch | Follow guidelines strictly, TestFlight early in Phase 4 |
| Combat feel doesn't match Dota2 | Core value prop fails | Phase 1 dedicated entirely to this, replay comparison tooling |
| Solo dev burnout | Project stalls | Realistic timeline, MVP subset, heavy agent assistance |
| Unity–Rust FFI issues on iOS | Blocks mobile | Prototype FFI bridge in Phase 0 week 1, test on device early |
| Networking complexity | Delays multiplayer | State-sync (simpler than lockstep), defer entirely to Phase 3 |

---

## Dependencies

| Dependency | When Needed | Notes |
|------------|-------------|-------|
| Rust stable + cross-compilation | Phase 0 | aarch64-apple-ios, aarch64-linux-android targets |
| Unity 6 LTS (6000.0) | Phase 0 | Long-term support, mobile build support |
| PostgreSQL | Phase 3+ | Player accounts, matchmaking, leaderboards |
| Cloud hosting | Phase 3+ | Game servers, matchmaking service |
| Apple Developer account | Phase 4 | $99/year, needed for TestFlight and App Store |
| Art assets (AI-generated) | Phase 4 | Characters, VFX, UI elements |
