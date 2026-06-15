set shell := ["bash", "-cu"]

mod frontend
mod backend

compose := "docker compose -f docker/docker-compose.yml"

# List available recipes
default:
    @just --list

# --- Build -------------------------------------------------------------------

build:
    just frontend::build
    just backend::build

# --- Format ------------------------------------------------------------------

fmt:
    just frontend::fmt
    just backend::fmt

fmt-check:
    just frontend::fmt-check
    just backend::fmt-check

# --- Lint ----------------------------------------------------------------------

lint:
    just frontend::lint
    just backend::lint

# --- Unit tests ------------------------------------------------------------------

test:
    just frontend::test
    just backend::test

# --- Functional tests --------------------------------------------------------------

test-functional:
    just frontend::test-functional
    just backend::test-functional

# --- Integration tests ----------------------------------------------------------------

test-integration:
    just frontend::test-integration
    just backend::test-integration

# --- Dead code -----------------------------------------------------------------------

dead-code:
    just frontend::dead-code
    just backend::dead-code

# --- Dependency updates / vulnerabilities -------------------------------------------------

deps-check:
    just frontend::deps-check
    just backend::deps-check

# --- Run locally -----------------------------------------------------------------------------

# Run backend and frontend dev servers together
run:
    just backend::run &
    just frontend::run

# --- Docker --------------------------------------------------------------------------------

docker-build:
    {{compose}} build

docker-up:
    {{compose}} up

docker-down:
    {{compose}} down

# --- CI --------------------------------------------------------------------------------------

# Full CI pipeline: format check, lint, build, unit/functional/integration tests, dead code, deps audit
ci: fmt-check lint build test test-functional test-integration dead-code deps-check
