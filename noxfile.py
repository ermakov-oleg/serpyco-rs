import os

import nox

nox.options.sessions = ["test", "lint", "type_check"]
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
    paths = [".", "../../tests"]
    session.run("black", *(["--check", "--diff", *paths] if _is_ci() else paths))
    session.run("isort", *(["--check", "--diff", *paths] if _is_ci() else paths))
    session.run("ruff", ".")


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
        "--benchmark-min-time=1",
        "--benchmark-max-time=5",
        "--benchmark-disable-gc",
        "--benchmark-autosave",
        "--benchmark-save-data",
        "bench",
    )


def _is_ci() -> bool:
    return bool(os.environ.get("CI", None))
