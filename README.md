# AA2 — Ability Arena 2

<!-- Badges -->
![Status](https://img.shields.io/badge/status-Phase%200%20Foundation-blue)
![Rust](https://img.shields.io/badge/rust-stable-orange)
![License](https://img.shields.io/badge/license-MIT-green)

A standalone cross-platform autobattler inspired by the Dota 2 mod Ability Arena. Eight players compete in a free-for-all, picking gods, drafting hero bodies, and equipping abilities to outlast their opponents.

## Status: Phase 0 (Foundation)

Core simulation crate scaffolding and data schema design.

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Simulation | Rust (`aa2-sim`, `aa2-data` crates) |
| Client | Unity 6 LTS (URP 2D) |
| Server | Rust (`aa2-server`, Phase 3) |
| Networking | WebSocket, state-sync at 10 Hz |
| Data | RON files (dev) / PostgreSQL JSONB (production) |

## Project Structure

```
aa2/
├── crates/
│   ├── aa2-sim/        # Deterministic combat simulation
│   ├── aa2-data/       # Shared types, schemas, RON loaders
│   └── aa2-server/     # Authoritative game server (Phase 3)
├── client/             # Unity 6 project (URP 2D)
├── data/               # RON data files (gods, abilities, bodies)
├── docs/               # Architecture & design documentation
├── tests/              # Integration tests
└── README.md
```

## Getting Started

### Prerequisites

- **Rust** (stable, latest) — [rustup.rs](https://rustup.rs)
- **Unity 6 LTS** (6000.0+) — for client work only

### Build

```bash
cargo build
cargo test
```

### Run Dev Mode (coming soon)

Local simulation runner with placeholder art and CLI output.

## Game Overview

1. **God Pick** — Each player selects a god that grants a passive bonus for the match.
2. **Draft Phase** — Players draft hero bodies and abilities from a shared pool.
3. **Equip** — Assign abilities to hero body slots, building synergies.
4. **Combat** — Automated round-robin battles between player boards.
5. **Elimination** — Players lose HP on defeat; last player standing wins.

Matches support 8 players with rounds of increasing intensity.

## Documentation

| Document | Description |
|----------|-------------|
| [docs/architecture.md](docs/architecture.md) | Technical architecture & system design |
| [docs/project-plan.md](docs/project-plan.md) | Phased development plan |
| [docs/mechanics-reference.md](docs/mechanics-reference.md) | Engine formulas & combat mechanics |

## Contributing

This project is in early development. Contribution guidelines will be published once the foundation stabilizes.

## License

[MIT](LICENSE)
