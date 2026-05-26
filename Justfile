set shell := ["bash", "-euo", "pipefail", "-c"]

uv := env_var_or_default("UV", "uv")

# Show available recipes
default:
    @just --list

# === Setup (one runs before checks) ===

# Local: install dev deps + rebuild extension via maturin
build:
    {{uv}} sync --group dev --no-install-project --inexact
    {{uv}} run --no-sync maturin develop --release

# Note: `uv sync --no-install-project` installs runtime dependencies but skips the project itself;
# `uv pip install` is required because `uv sync` would rebuild the project from source
# via its build-backend, ignoring local wheels.
# CI: install dev deps + pre-built wheel from ./wheels
install-wheel:
    {{uv}} sync --group dev --no-install-project --inexact
    {{uv}} pip install --no-index --no-deps --find-links wheels --reinstall serpyco-rs

# === Reusable checks ===
#
# `uv run --no-sync` is mandatory here: a plain `uv run` would re-install the
# project from source as editable, overwriting the wheel set up by `install-wheel`.

_run-tests:
    {{uv}} sync --group dev --no-install-project --inexact
    {{uv}} run --no-sync pytest -vvs tests/

_run-lint mode="fix":
    {{uv}} sync --group lint --no-install-project --inexact
    cd python/serpyco_rs && {{uv}} run --no-sync ruff format {{ if mode == "check" { "--check --diff" } else { "" } }} . ../../tests ../../bench
    cd python/serpyco_rs && {{uv}} run --no-sync ruff check {{ if mode == "fix" { "--fix" } else { "" } }} .

_run-type-check:
    {{uv}} sync --group type_check --no-install-project --inexact
    cd python/serpyco_rs && {{uv}} run --no-sync pyright .
    cd python/serpyco_rs && {{uv}} run --no-sync pyright . --verifytypes serpyco_rs
    cd python/serpyco_rs && {{uv}} run --no-sync mypy . --strict --implicit-reexport --pretty

_run-bench:
    {{uv}} sync --group bench-compare --no-install-project --inexact
    {{uv}} run --no-sync pytest bench --verbose \
        --benchmark-min-time=0.5 --benchmark-max-time=1 \
        --benchmark-disable-gc --benchmark-autosave \
        --benchmark-save-data --benchmark-compare

_rust-fmt mode="fix":
    cargo fmt --all {{ if mode == "check" { "-- --check" } else { "" } }}

_rust-clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# === Local entry points (rebuild then check) ===

# Run pytest
test: build _run-tests

# Format + lint Python code
lint: (_run-lint "fix")

# pyright + mypy
type-check: build _run-type-check

# Run benchmarks (with competitors)
bench: build _run-bench

# cargo fmt + clippy
rust-lint: (_rust-fmt "fix") _rust-clippy

# === CI entry points (use pre-built wheel, no auto-fix) ===

ci-test: install-wheel _run-tests
ci-lint: (_run-lint "check")
ci-type-check: install-wheel _run-type-check
ci-bench: install-wheel _run-bench
ci-rust-fmt: (_rust-fmt "check")
ci-rust-clippy: _rust-clippy

# === Special ===

# No `ci-coverage` variant — instrumented Rust coverage requires building from source
# with `cargo-llvm-cov` env vars set, so a pre-built wheel from artifacts won't work.
# CI invokes `just coverage` directly.
# Combined Python + Rust coverage
coverage:
    #!/usr/bin/env bash
    set -euo pipefail
    command -v lcov >/dev/null || { echo "lcov required: brew/apt install lcov" >&2; exit 1; }
    command -v genhtml >/dev/null || { echo "lcov required: brew/apt install lcov" >&2; exit 1; }
    {{uv}} sync --group dev --no-install-project --inexact
    mkdir -p coverage
    eval "$({{uv}} run --no-sync cargo llvm-cov show-env --release --sh)"
    {{uv}} run --no-sync cargo llvm-cov clean --workspace
    {{uv}} run --no-sync maturin develop --release
    {{uv}} run --no-sync coverage erase
    {{uv}} run --no-sync coverage run -m pytest -vvs tests/
    {{uv}} run --no-sync coverage lcov -o coverage/python.lcov
    {{uv}} run --no-sync coverage report
    {{uv}} run --no-sync cargo llvm-cov report --release --lcov --output-path coverage/rust.lcov
    lcov --add-tracefile coverage/python.lcov \
        --add-tracefile coverage/rust.lcov \
        --output-file coverage/lcov.info \
        --ignore-errors inconsistent,corrupt
    genhtml coverage/lcov.info --output-directory coverage/html \
        --title 'serpyco-rs coverage' \
        --ignore-errors inconsistent,corrupt,category
    echo "Combined: coverage/lcov.info"
    echo "HTML: coverage/html/index.html"

# Reference-count leak detection (requires debug Python build)
_run-test-rc-leaks:
    {{uv}} sync --group bench-compare --no-install-project --inexact
    {{uv}} run --no-sync pytest bench --verbose --debug-refs --debug-refs-gc

test-rc-leaks: build _run-test-rc-leaks
ci-test-rc-leaks: install-wheel _run-test-rc-leaks

# CI PGO: install PGO-instrumented wheel + bench deps, run targeted benches to gather profile data
ci-pgo-collect wheel_dir="pgo-wheel":
    {{uv}} sync --group pgo --no-install-project --inexact
    {{uv}} pip install --no-index --no-deps --find-links {{wheel_dir}} --reinstall serpyco_rs
    {{uv}} run --no-sync pytest bench/test_encoders.py bench/test_flatten.py bench/test_full.py bench/compare/test_github_issue.py -k "not mashumaro"

# Setup environment for pytest-codspeed (deps only; runner is invoked via the CodSpeed action)
_bench-codspeed-setup:
    {{uv}} sync --group codspeed --no-install-project --inexact

# Assumes deps are already synced by a preceding `_bench-codspeed-setup` /
# `ci-bench-codspeed-setup` step (CodSpeed action wraps just the runner).
# Run pytest under pytest-codspeed instrumentation
bench-codspeed-run:
    {{uv}} run --no-sync pytest bench --ignore=bench/compare/test_benchmarks.py --codspeed

bench-codspeed: build _bench-codspeed-setup bench-codspeed-run
ci-bench-codspeed-setup: install-wheel _bench-codspeed-setup

# Remove build artifacts
clean:
    rm -rf target/ coverage/ .pytest_cache/ .ruff_cache/
    find . -name '*.so' -delete
    find . -name '__pycache__' -type d -exec rm -rf {} +
