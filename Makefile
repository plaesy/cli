.PHONY: help build release test test-one lint fmt fmt-check bench eval lme docs clean model verify reload

help:
	@echo "Plaesy CLI - build targets"
	@echo ""
	@echo "  make build                 Debug build"
	@echo "  make release               Optimised build"
	@echo "  make reload                Build debug CLI and replace the user-installed binary"
	@echo "  make test                  Run all tests"
	@echo "  make test-one TEST=name    Run tests matching name"
	@echo "  make eval                  Relevance evaluation (Recall@3 / MRR), prints the table"
	@echo "  make lme DATA=file.json    LongMemEval-S retrieval benchmark (STRIDE=5 for every 5th question)"
	@echo "  make bench                 Criterion benchmarks"
	@echo "  make lint                  clippy with -D warnings"
	@echo "  make fmt / fmt-check       Format / check formatting"
	@echo "  make model                 Download the ONNX embedding model (one-time)"
	@echo "  make verify                build + test + lint + fmt-check"

build:
	cargo build --locked

release:
	cargo build --release

test:
	cargo test --all --locked

test-one:
	cargo test --all --locked $(TEST)

eval:
	cargo test --test relevance_eval --locked -- --nocapture

bench:
	cargo bench --locked

STRIDE ?= 1
lme:
	@test -n "$(DATA)" || { echo "usage: make lme DATA=longmemeval_s_cleaned.json [STRIDE=5]"; exit 2; }
	cargo run --release --locked --example longmemeval -- "$(DATA)" --stride $(STRIDE)

lint:
	cargo clippy --all-targets --locked -- -D warnings

fmt:
	cargo fmt

fmt-check:
	cargo fmt -- --check

# Cargo.lock is intentionally kept (this crate ships binaries).
clean:
	cargo clean

docs:
	cargo doc --no-deps --open

model:
	./scripts/install-model.sh

verify: build test lint fmt-check
	@echo "All checks passed"


ifeq ($(OS),Windows_NT)
CARGO_HOME ?= $(USERPROFILE)/.cargo
CARGO ?= "$(CARGO_HOME)/bin/cargo.exe"
else
CARGO ?= cargo
endif

reload:
	$(CARGO) build --locked --bin plaesy
	$(CARGO) run --locked --bin plaesy -- install

.DEFAULT_GOAL := help
