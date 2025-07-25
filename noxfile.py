import os
import sys

import nox


nox.options.sessions = ['test', 'lint', 'type_check', 'rust_lint']


def build(session, use_pip: bool = False):
    if _is_ci():
        # Install form wheels
        install(session, '--no-index', '--no-deps', '--find-links', 'wheels/', 'serpyco-rs', use_pip=use_pip)
        install(session, '--find-links', 'wheels/', 'serpyco-rs', use_pip=use_pip)
        return

    session.run_always('maturin', 'develop', '-r')


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


def install(session, *args, use_pip: bool = False):
    if session._runner.global_config.no_install:
        return
    cmd = ['pip', 'install'] if use_pip else ['uv', 'pip', 'install', '--python', sys.executable]
    session.run_always(*cmd, *args)


def _is_ci() -> bool:
    return bool(os.environ.get('CI', None))
