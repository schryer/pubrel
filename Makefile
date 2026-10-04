# The shared targets (venv, lock, fmt, lint, test, doc, build, functional)
# come from pubkit.mk, which pubkit manages; this file adds pubrel's own.
include pubkit.mk

.DEFAULT_GOAL := help
export PUBREL_BIN_DIR ?= $(CURDIR)/target/debug
# The released pub the suite drives; override to point elsewhere.
export PUB_BIN_DIR ?= $(dir $(shell command -v pub 2>/dev/null || echo $(HOME)/.cargo/bin/pub))

.PHONY: check clean

check: sync-check fmt-check lint test doc functional ## Everything CI runs

clean: ## Remove build artifacts
	cargo clean
	rm -rf .pytest_cache tests/.pytest_cache
