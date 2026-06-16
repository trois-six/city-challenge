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
