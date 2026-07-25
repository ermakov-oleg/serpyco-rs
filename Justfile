set shell := ["bash", "-euo", "pipefail", "-c"]

uv := env_var_or_default("UV", "uv")
venv := env_var_or_default("UV_PROJECT_ENVIRONMENT", ".venv")

# Show available recipes
default:
    @just --list

# === Environment primitives ===

# Sync one dependency group without installing the project itself.
_sync group:
    {{uv}} sync --group {{group}} --no-install-project --inexact

# CI syncs must match uv.lock exactly.
_sync-ci group:
    {{uv}} sync --locked --group {{group}} --no-install-project

# Install a pre-built wheel into the project virtualenv created by uv sync.
_install-wheel wheel_dir="wheels":
    {{uv}} pip install --python {{venv}} --no-index --no-deps --find-links {{wheel_dir}} --reinstall serpyco-rs

# === Setup ===

# Local: install dev deps + rebuild extension via maturin
build: (_sync "dev")
    {{uv}} run --no-sync maturin develop --release

# Note: `uv sync --no-install-project` installs runtime dependencies but skips
# the project itself; `uv pip install` is required because `uv sync` would
# rebuild the project from source via its build-backend, ignoring local wheels.
# CI: install dev deps + pre-built wheel from ./wheels
install-wheel: (_sync "dev") (_install-wheel "wheels")

# === Reusable checks ===
#
# `uv run --no-sync` is mandatory here: a plain `uv run` would re-install the
# project from source as editable, overwriting the wheel set up by `install-wheel`.

_run-tests args="tests/":
    {{uv}} run --no-sync pytest -vvs {{args}}

_run-lint mode="fix":
    cd python/serpyco_rs && {{uv}} run --no-sync ruff format {{ if mode == "check" { "--check --diff" } else { "" } }} . ../../tests ../../bench
    cd python/serpyco_rs && {{uv}} run --no-sync ruff check {{ if mode == "fix" { "--fix" } else { "" } }} .

_run-type-check:
    PYTHONPATH=python {{uv}} run --no-sync pyright python/serpyco_rs tests/test_codec_typing.py
    PYTHONPATH=python {{uv}} run --no-sync pyright --verifytypes serpyco_rs
    PYTHONPATH=python {{uv}} run --no-sync mypy python/serpyco_rs tests/test_codec_typing.py --strict --implicit-reexport --enable-incomplete-feature=TypeForm --pretty

_run-bench target="bench":
    {{uv}} run --no-sync pytest {{target}} --verbose \
        --benchmark-min-time=0.5 --benchmark-max-time=1 \
        --benchmark-disable-gc --benchmark-autosave \
        --benchmark-save-data --benchmark-compare

_rust-fmt mode="fix":
    cargo fmt --all {{ if mode == "check" { "-- --check" } else { "" } }}

_rust-clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# === Local entry points (rebuild then check) ===

# Run pytest
test args="tests/": build (_run-tests args)

# Format + lint Python code
lint: (_sync "lint") (_run-lint "fix")

# pyright + mypy over the source tree; no native rebuild required.
type-check: (_sync "type_check") _run-type-check

# Run benchmarks (with competitors)
bench target="bench": build (_sync "bench-compare") (_run-bench target)

# cargo fmt + clippy
rust-lint: (_rust-fmt "fix") _rust-clippy

# === CI entry points (use pre-built wheel, no auto-fix) ===

ci-test args="tests/": (_sync-ci "dev") (_install-wheel "wheels") (_run-tests args)

ci-lint: (_sync-ci "lint") (_run-lint "check")

ci-type-check: (_sync-ci "type_check") _run-type-check

ci-bench target="bench": (_sync-ci "bench-compare") (_install-wheel "wheels") (_run-bench target)

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
_run-test-rc-leaks target="bench":
    {{uv}} run --no-sync pytest {{target}} --verbose --debug-refs --debug-refs-gc

test-rc-leaks target="bench": build (_sync "bench-compare") (_run-test-rc-leaks target)

ci-test-rc-leaks target="bench": (_sync-ci "bench-compare") (_install-wheel "wheels") (_run-test-rc-leaks target)

# CI PGO: install PGO-instrumented wheel + bench deps, run targeted benches to gather profile data.
# The codec (bytes) benches must be here too: code left out of the profile is
# compiled as cold — measured at -21%..-35% on the codec path when it is missing.
# Competitor benches are deselected; only serpyco-rs code belongs in the profile.
ci-pgo-collect wheel_dir="pgo-wheel": (_sync-ci "pgo") (_install-wheel wheel_dir)
    {{uv}} run --no-sync pytest \
        bench/test_encoders.py bench/test_codec_encoders.py \
        bench/test_flatten.py bench/test_full.py \
        bench/compare/test_github_issue.py bench/compare/test_github_issue_bytes.py \
        bench/compare/test_benchmarks_bytes.py \
        -k "not mashumaro and not msgspec" \
        --benchmark-min-time=0.2 --benchmark-max-time=0.4

# Setup environment for pytest-codspeed (deps only; runner is invoked via the CodSpeed action)
_bench-codespeed-setup: (_sync "codspeed")

# Assumes deps are already synced by a preceding `_bench-codespeed-setup` /
# `ci-bench-codespeed-setup` step (CodSpeed action wraps just the runner).
# Run pytest under pytest-codspeed instrumentation
bench-codespeed-run:
    {{uv}} run --no-sync pytest bench --ignore=bench/compare/test_benchmarks.py --codspeed

bench-codespeed: build _bench-codespeed-setup bench-codespeed-run
ci-bench-codespeed-setup: (_sync-ci "codspeed") (_install-wheel "wheels")

# Remove build artifacts
clean:
    rm -rf target/ coverage/ .pytest_cache/ .ruff_cache/
    find . -name '*.so' -delete
    find . -name '__pycache__' -type d -exec rm -rf {} +
