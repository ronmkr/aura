# Aura Makefile

.PHONY: all build test check fmt clippy clean docs run-cli run-daemon run-tui modularity-check audit bench setup-hooks

# Default: build everything
all: build

# --- Compilation ---

build:
	cargo build --workspace

release:
	cargo build --workspace --release

# --- Quality Control (The Green Loop) ---

check:
	cargo check --workspace

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

clippy:
	cargo clippy --workspace -- -D warnings

test:
	cargo test --workspace

test-cucumber:
	cd aura-core && cargo test --test cucumber

bench:
	cargo bench --workspace --no-run

audit:
	cargo audit

modularity-check:
	bash scripts/check_file_length.sh

setup-hooks:
	bash scripts/setup_hooks.sh

# The strict mandate for every commit
green-loop: fmt clippy test modularity-check bench


# --- Documentation ---

# Build the User Manual (requires mdbook)
docs:
	rm -rf aura-docs/manual/src/adr
	mkdir -p aura-docs/manual/src/adr
	cp aura-docs/adr/*.md aura-docs/manual/src/adr/
	cd aura-docs/manual && mdbook build

# Build the Rust API documentation
docs-api:
	cargo doc --workspace --no-deps --document-private-items

# Build both Manual and API docs
docs-all: docs docs-api

# Serve the manual locally
docs-serve:
	rm -rf aura-docs/manual/src/adr
	mkdir -p aura-docs/manual/src/adr
	cp aura-docs/adr/*.md aura-docs/manual/src/adr/
	cd aura-docs/manual && mdbook serve

# --- Execution ---

# Run the CLI (Usage: make run-cli ARGS="https://example.com/file")
run-cli:
	cargo run -p aura -- $(ARGS)

# Run the background daemon
run-daemon:
	cargo run -p aura -- daemon

# Run the TUI dashboard
run-tui:
	cargo run -p aura -- tui

# --- Cleanup ---

clean:
	bash scripts/clean_cache.sh
	rm -rf aura-docs/manual/book
