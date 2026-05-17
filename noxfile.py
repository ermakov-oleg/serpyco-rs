import os
import shlex
import shutil
import sys
from pathlib import Path

import nox
from nox.command import CommandFailed


nox.options.sessions = ['test', 'lint', 'type_check', 'rust_lint']

COVERAGE_DIR = Path('coverage')
WHEEL_DIR = COVERAGE_DIR / 'wheels'
PYTHON_COVERAGE_LCOV = COVERAGE_DIR / 'python.lcov'
RUST_COVERAGE_LCOV = COVERAGE_DIR / 'rust.lcov'
COMBINED_COVERAGE_LCOV = COVERAGE_DIR / 'lcov.info'
COVERAGE_HTML_DIR = COVERAGE_DIR / 'html'


def build(session, use_pip: bool = False, env=None):
    if _is_ci():
        # Install form wheels
        install(session, '--no-index', '--no-deps', '--find-links', 'wheels/', 'serpyco-rs', use_pip=use_pip)
        install(session, '--find-links', 'wheels/', 'serpyco-rs', use_pip=use_pip)
        return

    session.run_always('maturin', 'develop', '-r', env=env)


@nox.session(python=False)
def test(session):
    build(session)
    install(session, '-r', 'requirements/dev.txt')
    session.run('pytest', '-vvs', 'tests/', *session.posargs)


@nox.session(python=False)
def lint(session):
    build(session)
    install(session, '-r', 'requirements/lint.txt')

    session.cd('python/serpyco_rs')
    paths = ['.', '../../tests', '../../bench']
    session.run('ruff', 'format', *(['--check', '--diff', *paths] if _is_ci() else paths))
    session.run('ruff', 'check', '.', *([] if _is_ci() else ['--fix']))


@nox.session(python=False)
def rust_lint(session):
    session.run('cargo', 'fmt', '--all', *(['--', '--check'] if _is_ci() else []))
    session.run('cargo', 'clippy', '--all-targets', '--all-features', '--', '-D', 'warnings')


@nox.session(python=False)
def type_check(session):
    build(session)
    install(session, '-r', 'requirements/type_check.txt')

    session.cd('python/serpyco_rs')
    session.run('pyright', '.', success_codes=[0, 1] if _is_ci() else [0])
    session.run('pyright', '.', '--verifytypes', 'serpyco_rs')
    session.run('mypy', '.', '--strict', '--implicit-reexport', '--pretty')


@nox.session(python=False)
def bench(session):
    build(session)
    install(session, '-r', 'requirements/bench.txt')

    session.run(
        'pytest',
        *(session.posargs if session.posargs else ['bench']),
        '--verbose',
        '--benchmark-min-time=0.5',
        '--benchmark-max-time=1',
        '--benchmark-disable-gc',
        '--benchmark-autosave',
        '--benchmark-save-data',
        '--benchmark-compare',
    )


@nox.session(python=False)
def test_rc_leaks(session):
    # uv don't resolve wheels when used python debug build
    build(session, use_pip=True)
    install(session, '-r', 'requirements/bench.txt', use_pip=True)
    session.run(
        'pytest',
        *(session.posargs if session.posargs else ['bench']),
        '--verbose',
        '--debug-refs',
        '--debug-refs-gc',
    )


@nox.session(python=False)
def bench_codespeed(session):
    build(session)
    install(session, '-r', 'requirements/bench.txt')
    install(session, 'pytest-codspeed')
    session.run('pytest', 'bench', '--ignore=bench/compare/test_benchmarks.py', '--codspeed')


@nox.session(python=False)
def coverage(session):
    if sys.platform == 'win32':
        session.error('The coverage session is only supported on Unix-like systems.')

    install(session, '-r', 'requirements/dev.txt')

    _ensure_lcov(session)
    coverage_env = _cargo_llvm_cov_env(session)
    session.run('cargo', 'llvm-cov', 'clean', '--workspace')

    COVERAGE_DIR.mkdir(exist_ok=True)
    shutil.rmtree(WHEEL_DIR, ignore_errors=True)
    session.run_always('maturin', 'build', '-r', '--out', str(WHEEL_DIR), env=coverage_env)
    wheels = sorted(WHEEL_DIR.glob('*.whl'), key=lambda path: path.stat().st_mtime)
    if not wheels:
        session.error(f'No wheels found in {WHEEL_DIR}')
    install(session, '--force-reinstall', str(wheels[-1]))

    session.run('coverage', 'erase')
    session.run('coverage', 'run', '-m', 'pytest', '-vvs', 'tests/', *session.posargs, env=coverage_env)
    session.run('coverage', 'lcov', '-o', str(PYTHON_COVERAGE_LCOV))
    session.run('coverage', 'report')
    session.run(
        'cargo',
        'llvm-cov',
        'report',
        '--release',
        '--lcov',
        '--output-path',
        str(RUST_COVERAGE_LCOV),
        env=coverage_env,
    )
    session.run(
        'lcov',
        '--add-tracefile',
        str(PYTHON_COVERAGE_LCOV),
        '--add-tracefile',
        str(RUST_COVERAGE_LCOV),
        '--output-file',
        str(COMBINED_COVERAGE_LCOV),
        '--ignore-errors',
        'inconsistent,corrupt',
    )
    session.run(
        'genhtml',
        str(COMBINED_COVERAGE_LCOV),
        '--output-directory',
        str(COVERAGE_HTML_DIR),
        '--title',
        'serpyco-rs coverage',
        '--ignore-errors',
        'inconsistent,corrupt,category',
    )

    session.log(f'Combined coverage report: {COMBINED_COVERAGE_LCOV}')
    session.log(f'HTML coverage report: {COVERAGE_HTML_DIR / "index.html"}')


def install(session, *args, use_pip: bool = False):
    if session._runner.global_config.no_install:
        return
    cmd = ['pip', 'install'] if use_pip else ['uv', 'pip', 'install', '--python', sys.executable]
    session.run_always(*cmd, *args)


def _is_ci() -> bool:
    return bool(os.environ.get('CI', None))


def _ensure_lcov(session) -> None:
    if shutil.which('lcov') is None or shutil.which('genhtml') is None:
        session.error('lcov is required. Install it with `brew install lcov` or `apt-get install lcov`')


def _cargo_llvm_cov_env(session) -> dict[str, str]:
    try:
        output = session.run('cargo', 'llvm-cov', 'show-env', '--sh', silent=True)
    except CommandFailed:
        session.error('cargo-llvm-cov is required. Install it with `cargo install cargo-llvm-cov`')

    env = {}
    for line in output.splitlines():
        line = line.strip()
        if not line.startswith('export '):
            continue

        key, raw_value = line.removeprefix('export ').rstrip(';').split('=', 1)
        values = shlex.split(raw_value)
        env[key] = values[0] if values else ''

    return env
