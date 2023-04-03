import os

import nox

nox.options.sessions = ["test", "lint", "type_check", "rust_lint"]
nox.options.reuse_existing_virtualenvs = True


def build(session):
    if _is_ci():
        # Install form wheels
        session.install("--no-index", "--no-deps", "--find-links", "wheels/", "serpyco-rs")
        session.install("--find-links", "wheels/", "serpyco-rs")
        return

    session.run_always("maturin", "develop", "-r")


@nox.session
def test(session):
    build(session)
    session.install("-r", "requirements/dev.txt")
    session.run("pytest", "-vss", "tests/")


@nox.session
def lint(session):
    build(session)
    session.install("-r", "requirements/lint.txt")

    session.cd("python/serpyco_rs")
    paths = [".", "../../tests", "../../bench"]
    session.run("black", *(["--check", "--diff", *paths] if _is_ci() else paths))
    session.run("isort", *(["--check", "--diff", *paths] if _is_ci() else paths))
    session.run("ruff", ".")


@nox.session(python=False)
def rust_lint(session):
    session.run("cargo", "fmt", "--all", *(["--", "--check"] if _is_ci() else []))
    session.run("cargo", "clippy", "--all-targets", "--all-features", "--", "-D", "warnings")


@nox.session
def type_check(session):
    build(session)
    session.install("-r", "requirements/type_check.txt")

    session.cd("python/serpyco_rs")
    session.run("pyright")
    session.run("pyright", "--verifytypes", "serpyco_rs")
    session.run("mypy", ".", "--strict", "--implicit-reexport", "--pretty")


@nox.session
def bench(session):
    build(session)
    session.install("-r", "requirements/bench.txt")

    session.run(
        "pytest",
        "--verbose",
        "--benchmark-min-time=0.25",
        "--benchmark-max-time=1.5",
        "--benchmark-disable-gc",
        "--benchmark-autosave",
        "--benchmark-save-data",
        "--benchmark-compare",
        "--ignore=bench/test_full.py",
        *(session.posargs if session.posargs else ["bench"]),
    )


@nox.session
def bench_codespeed(session):
    build(session)
    session.install("-r", "requirements/bench.txt")
    session.install('pytest-codspeed')
    session.run("pytest", "bench", "--ignore=bench/compare", "--codspeed")


def _is_ci() -> bool:
    return bool(os.environ.get("CI", None))
