import contextlib
import gc
import pprint
import sys
from typing import Any, Callable

import pytest


def repeat(func: Callable[[], Any], count: int = 1000) -> Callable[[], Any]:
    def inner():
        for _i in range(count):
            func()

    return inner


@contextlib.contextmanager
def check_refcount(tolerance: int = 1000):
    """Check that refcount of gc tracked objects is the same before and after the test."""

    def _get_ref_counts() -> dict[str, int]:
        return {
            repr(o)[:50]: count
            for o in (*gc.get_objects(0), *gc.get_objects(1), *gc.get_objects(2))
            if (count := sys.getrefcount(o)) > 500
        }

    before = _get_ref_counts()
    yield
    after = _get_ref_counts()

    changes = {}
    for key, value in after.items():
        if abs(value - before.get(key, 0)) > tolerance:
            changes[key] = value - before.get(key, 0)

    if changes:
        pytest.fail(f'refcount changed: \n{pprint.pformat(changes)}')
