.PHONY: help deps dev-server dev-ui dev build test test-rust test-react \
	lint clippy validate format format-check build-web clean \
	schema-doc

# Ports
SERVER_PORT ?= 8080

# Paths
ROOT        := $(shell pwd)
PRESET_REACT    := $(ROOT)/packages/preset_react

# Data directory (path to a cloned audiflow-preset data repo)
DATA_DIR    ?= $(ROOT)/../audiflow-preset

# Vite env: point React dev server at the API
export VITE_API_BASE_URL ?= http://localhost:$(SERVER_PORT)

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-16s\033[0m %s\n", $$1, $$2}'

# -- Setup -------------------------------------------------------------------

deps: ## Install dependencies for all packages
	cd $(PRESET_REACT) && pnpm install

# -- Run services ------------------------------------------------------------

dev-server: ## Start the backend API server (PORT=$(SERVER_PORT))
	cargo run -- serve --data-dir $(DATA_DIR) --port $(SERVER_PORT)

dev-ui: ## Start React web app dev server
	cd $(PRESET_REACT) && pnpm dev

dev: ## Start server and React web app together (Ctrl+C stops both)
	@trap 'kill 0 2>/dev/null' EXIT; \
	cargo run -- serve --data-dir $(DATA_DIR) --port $(SERVER_PORT) & \
	echo "preset_server started on port $(SERVER_PORT)"; \
	cd $(PRESET_REACT) && pnpm dev

# -- Testing -----------------------------------------------------------------

test: ## Run all tests (Rust + React)
	cargo test
	cd $(PRESET_REACT) && pnpm test -- --run

test-rust: ## Run Rust tests
	cargo test

test-react: ## Run React tests
	cd $(PRESET_REACT) && pnpm test -- --run

# -- Quality -----------------------------------------------------------------

lint: ## Run all linters (clippy + oxlint + tsc)
	cargo clippy -- -W warnings
	cd $(PRESET_REACT) && npx oxlint
	cd $(PRESET_REACT) && npx tsc -b --noEmit

clippy: ## Run cargo clippy
	cargo clippy -- -W warnings

format: ## Format JSON configs in data directory
	cargo run -- format --data-dir $(DATA_DIR)

format-check: ## Check JSON formatting without applying changes
	cargo run -- format --data-dir $(DATA_DIR) --check

# -- Build -------------------------------------------------------------------

build: build-web ## Build React SPA + Rust release binary
	cargo build --release

build-web: ## Build React SPA for production
	cd $(PRESET_REACT) && pnpm build

# -- Validation --------------------------------------------------------------

validate: ## Validate configs in data directory against schema
	cargo run -- validate --data-dir $(DATA_DIR)

# -- Schema ------------------------------------------------------------------

schema-doc: ## Regenerate schema HTML docs from local schema files
	bash scripts/generate_schema_doc.sh

# -- Cleanup -----------------------------------------------------------------

clean: ## Remove build artifacts and caches
	rm -rf $(PRESET_REACT)/dist $(PRESET_REACT)/node_modules
	cargo clean
