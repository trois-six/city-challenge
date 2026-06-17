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

# --- Town data ---------------------------------------------------------------

# Build/update one or more towns (or every town in cities.json if IDS is
# empty). Add a town by adding an entry to cities.json first, then run this
# with its id.
city-build IDS="":
    cargo run --release --bin build-data -- {{IDS}}

# Delete a town's generated data. The town's entry must still be in
# cities.json when running this. Afterwards remove the entry and run
# `just backend::city-build` to refresh the global manifests.
city-delete ID:
    cargo run --release --bin build-data -- --delete {{ID}}
