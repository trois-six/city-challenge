# City Challenge

Every street. Every corner. Every city.

What if your city became your playground — and every run, walk, or ride a step closer to glory?

City Challenge is the ultimate urban exploration game that turns the streets you live on into a competitive arena. The goal is simple yet addictive: cover every single street of a city, as fast as possible. Track your progress, climb the leaderboard, and prove you know your city better than anyone else.

Whether you're a seasoned runner, a casual cyclist, or just someone who loves to wander — City Challenge transforms your everyday movement into an epic quest. Earn rewards as you push your completion percentage higher, unlock achievements as you conquer new cities, and compete against explorers from around the world.

One city at a time. One street at a time. The map is waiting. The clock is ticking. The challenge is yours.

## Architecture

- **`frontend/`** — React + Vite + TypeScript single-page app
  (`react-i18next` for FR/EN, CSS Modules, `react-router-dom`). It is fully
  static: all data is read from JSON files under `frontend/public/data/`.
- **`backend/`** — Rust workspace with two binaries:
  - `server` — an Actix Web API (`/api/*`), currently serving mock data; the
    frontend does not depend on it for content.
  - `build-data` — the data pipeline. For each configured city it fetches
    street data from OpenStreetMap (Overpass API), falls back to a
    deterministic synthetic street grid if Overpass is unreachable, computes
    the optimal route covering every street (`backend/src/route.rs`), and
    writes the manifests consumed by the frontend under
    `frontend/public/data/`.

## Requirements

- [Rust](https://www.rust-lang.org/) (stable) + Cargo
- [Node.js](https://nodejs.org/) + npm
- [just](https://github.com/casey/just) — task runner used for all
  development commands
- Docker + Docker Compose (optional, for containerized runs)

## [Documentation](/docs/howto.md)

## License

MIT — see [LICENSE](LICENSE).
