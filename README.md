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
- **`docker/`** — Dockerfiles and a `docker-compose.yml` for running the
  frontend, backend and Postgres together.

## Requirements

- [Rust](https://www.rust-lang.org/) (stable) + Cargo
- [Node.js](https://nodejs.org/) + npm
- [just](https://github.com/casey/just) — task runner used for all
  development commands
- Docker + Docker Compose (optional, for containerized runs)

## Getting started

All common tasks are exposed as `just` recipes from the repository root. Run
`just --list` to see everything, or `just --list <module>` for one side
only (e.g. `just --list frontend`).

```sh
# Run the frontend dev server and backend API together
just run

# Or individually
just frontend::run
just backend::run
```

### Build

```sh
just build              # frontend + backend
just frontend::build
just backend::build
```

### Format & lint

```sh
just fmt                # apply formatting/fixes
just fmt-check          # check only (used in CI)
just lint
```

### Tests

```sh
just test               # unit tests (frontend + backend)
just test-functional
just test-integration   # full Actix app stack / frontend data-layer contract
```

### Dead code & dependency checks

```sh
just dead-code          # cargo machete + knip
just deps-check         # cargo audit/outdated + npm audit/outdated
```

### Full CI pipeline

```sh
just ci                 # fmt-check, lint, build, all test tiers, dead-code, deps-check
```

## Data pipeline

Regenerate the static city data consumed by the frontend:

```sh
cd backend
cargo run --release --bin build-data            # all configured cities
cargo run --release --bin build-data -- FR-75001-Paris   # a single city
```

This writes per-city manifests (index, optimized route, stats/leaderboard)
under `frontend/public/data/`, plus the global `cities.json`, `players.json`
and `leaderboard.json` manifests.

## Docker

```sh
just docker-build
just docker-up
just docker-down
```

This builds and runs the frontend (port 3000), backend (port 8080) and a
Postgres database, as defined in `docker/docker-compose.yml`.

## Kubernetes

`docker/k8s/` contains manifests for a namespace, Postgres (with PVC),
backend and frontend Deployments/Services, and an Ingress. Apply them with:

```sh
kubectl apply -f docker/k8s/
```

The Deployments reference `ghcr.io/trois-six/city-challenge-{frontend,backend}:latest`,
published by the `Docker` GitHub Actions workflow.

## License

MIT — see [LICENSE](LICENSE).
