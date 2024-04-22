import gc
import sys

import pytest


def pytest_addoption(parser):
    parser.addoption(
        '--debug-refs',
        action='store_true',
        default=False,
        help='Check refcount of gc tracked objects before and after the test.',
    )
    parser.addoption(
        '--debug-refs-gc', action='store_true', default=False, help='Collect garbage before and after the test.'
    )


@pytest.fixture
def debug_refs(request):
    return request.config.getoption('--debug-refs')


@pytest.fixture
def debug_refs_gc(request):
    return request.config.getoption('--debug-refs-gc')


@pytest.fixture
def bench_or_check_refcount(benchmark, debug_refs, debug_refs_gc):
    if not debug_refs:
        return benchmark
    benchmark._mode = 'skip benchmark'

    if not hasattr(sys, 'gettotalrefcount'):
        raise RuntimeError('For use --debug-refs option, you need to run tests with python debug build.')

    def inner(fn, *args, tolerance=5, **kwargs):
        if debug_refs_gc:
            gc.collect()
        before = sys.gettotalrefcount()
        fn(*args, **kwargs)
        if debug_refs_gc:
            gc.collect()
        after = sys.gettotalrefcount()
        diff = after - before

        if abs(diff) > tolerance:
            message = f'[refcount changed] before: {before} after: {after} diff: {diff}'
            if inner.skip_refcount:
                pytest.skip(message)
            else:
                pytest.fail(message)

    inner.skip_refcount = False

    # for pytest-benchmark compatibility
    inner.extra_info = {}

    return inner
