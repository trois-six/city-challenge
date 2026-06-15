# Recipes run with the frontend directory as working directory.

# Install dependencies (matches package-lock.json exactly)
install:
    npm ci

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
