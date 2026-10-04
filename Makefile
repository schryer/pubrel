# Single entry point, mirroring publet and graphset.
SHELL := /bin/bash
.SHELLFLAGS := -eu -o pipefail -c
export PATH := $(HOME)/.cargo/bin:$(CURDIR)/.venv/bin:$(PATH)
export PUBREL_BIN_DIR ?= $(CURDIR)/target/debug
# The released pub the suite drives; override to point elsewhere.
export PUB_BIN_DIR ?= $(dir $(shell command -v pub 2>/dev/null || echo $(HOME)/.cargo/bin/pub))

.PHONY: help check fmt-check fmt lint test doc build functional lock venv clean

help: ## Show available targets
	@grep -E '^[a-z-]+:.*##' $(MAKEFILE_LIST) | awk -F':.*## ' '{printf "  %-12s %s\n", $$1, $$2}'

check: fmt-check lint test doc functional ## Everything CI runs

fmt: ## Format Rust sources
	cargo fmt --all

fmt-check: ## Verify formatting without modifying
	cargo fmt --all -- --check

lint: ## Clippy, warnings are errors
	cargo clippy --all-targets -- -D warnings

test: ## Rust unit tests
	cargo test

doc: ## Documentation, warnings are errors
	RUSTDOCFLAGS="-D warnings" cargo doc --no-deps

build: ## Debug build
	cargo build

venv: ## Python environment for the functional suite, hash-locked
	python3 -m venv .venv
	.venv/bin/pip install --quiet --require-hashes -r tests/requirements.lock

functional: build ## Gherkin suite against the built pubrel and a released pub
	cd tests && pytest -q

lock: ## Regenerate tests/requirements.lock from requirements.txt
	.venv/bin/pip install --quiet pip-tools
	.venv/bin/pip-compile --quiet --generate-hashes --output-file tests/requirements.lock tests/requirements.txt

clean: ## Remove build artifacts
	cargo clean
	rm -rf .pytest_cache tests/.pytest_cache
