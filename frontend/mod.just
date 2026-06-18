# Recipes run with the frontend directory as working directory.

# List available recipes
default:
    @just --list frontend

# Install dependencies (matches package-lock.json exactly).
# Skips npm ci when node_modules is already up-to-date so that recipes
# depending on this step are fast when nothing has changed.
install:
    @cmp -s node_modules/.package-lock.json package-lock.json 2>/dev/null || npm ci

build: install
    npm run build

fmt: install
    npm run lint:fix

fmt-check: install
    npm run lint

lint: install
    npm run lint

test: install
    npm test -- --run

# Convention: *.functional.test.{ts,tsx}
test-functional: install
    npx vitest run --passWithNoTests "functional.test"

# Convention: *.integration.test.{ts,tsx}
test-integration: install
    npx vitest run --passWithNoTests "integration.test"

dead-code: install
    npx knip

deps-check: install
    npm audit
    npm outdated || true

run: install
    npm run dev
