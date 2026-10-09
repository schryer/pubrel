# The shared targets (venv, lock, fmt, lint, test, doc, build, functional)
# come from pubkit.mk, which pubkit manages; this file adds pubrel's own.
include pubkit.mk

.DEFAULT_GOAL := help
export PUBREL_BIN_DIR ?= $(CURDIR)/target/debug
# The released pub the suite drives; override to point elsewhere.
export PUB_BIN_DIR ?= $(dir $(shell command -v pub 2>/dev/null || echo $(HOME)/.cargo/bin/pub))

.PHONY: check clean fuzz-smoke

check: sync-check fmt-check lint test doc functional ## Everything CI runs

clean: ## Remove build artifacts
	cargo clean
	rm -rf .pytest_cache tests/.pytest_cache

# What `pubrel check` reads from a pull request, fuzzed (fuzz/).
FUZZ_TARGETS := release_json unreleased_json corpus_lock manifest
FUZZ_SECONDS ?= 60

fuzz-smoke: ## Replay fuzz regressions, then fuzz each target briefly (needs nightly and cargo-fuzz)
	cd fuzz && cargo +nightly fuzz build
	cd fuzz && for target in $(FUZZ_TARGETS); do \
	  mkdir -p corpus/$$target regressions/$$target; \
	  cargo +nightly fuzz run $$target corpus/$$target regressions/$$target -- \
	    -max_total_time=$(FUZZ_SECONDS) -timeout=10 || exit 1; \
	done
