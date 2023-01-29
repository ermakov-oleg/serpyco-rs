import platform

import nox


@nox.session(reuse_venv=True)
def bench(session):
    session.install('-r', 'requirements/bench.txt')

    if platform.machine() == 'arm64' and platform.system() == 'Darwin':
        # https://github.com/Stranger6667/jsonschema-rs/issues/409
        session.install('jsonschema_rs', '--no-binary', ':all:')

    session.run('maturin', 'develop', '-r')
    session.run(
        'pytest',
        '--verbose',
        '--benchmark-min-time=1',
        '--benchmark-max-time=5',
        '--benchmark-disable-gc',
        '--benchmark-autosave',
        '--benchmark-save-data',
        'bench',
    )
