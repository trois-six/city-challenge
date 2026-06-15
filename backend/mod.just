# Recipes run with the backend directory as working directory.

build:
    cargo build --release

fmt:
    cargo fmt

fmt-check:
    cargo fmt --check

lint:
    cargo clippy --all-targets -- -D warnings

test:
    cargo test --lib

# Convention: backend/tests/functional_*.rs
test-functional:
    cargo test --tests --quiet -- functional

# Runs the full Actix app/router/handler stack via actix_web::test (mocked DB)
test-integration:
    cargo test --test integration_api

dead-code:
    cargo machete

deps-check:
    cargo audit
    cargo outdated || true

run:
    cargo run --bin server
