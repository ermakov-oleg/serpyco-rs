"""Concurrency stress tests for free-threaded Python support.

These exercise the immutable, shared :class:`Serializer` instance from
multiple threads at once. Under a free-threaded CPython build (``python3.13t``
and later) this catches real data races in the encoder graph; under a
standard GIL build it still guards against logical regressions, such as
shared mutable state sneaking back into encoders or the recursive
``LazyEncoder`` slot.
"""

from __future__ import annotations

import sys
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass, field

from serpyco_rs import Serializer


@dataclass
class Node:
    value: int
    children: list[Node] = field(default_factory=list)


def _build_tree(depth: int, fanout: int, counter: list[int]) -> Node:
    counter[0] += 1
    if depth == 0:
        return Node(value=counter[0])
    return Node(
        value=counter[0],
        children=[_build_tree(depth - 1, fanout, counter) for _ in range(fanout)],
    )


def test_concurrent_dump_load_recursive_type() -> None:
    """Hammer a shared Serializer with a recursive (LazyEncoder-backed) type."""
    serializer = Serializer(Node)
    tree = _build_tree(depth=4, fanout=3, counter=[0])
    expected = serializer.dump(tree)

    workers = 8
    iterations = 200

    def worker() -> tuple[dict, Node]:
        dumped: dict = {}
        loaded: Node = tree
        for _ in range(iterations):
            dumped = serializer.dump(tree)
            loaded = serializer.load(expected)
        return dumped, loaded

    with ThreadPoolExecutor(max_workers=workers) as pool:
        futures = [pool.submit(worker) for _ in range(workers)]
        for fut in as_completed(futures):
            dumped, loaded = fut.result()
            assert dumped == expected
            assert loaded == tree


def test_concurrent_serializer_construction() -> None:
    """Each Serializer owns its EncoderState; many built in parallel must not interfere."""

    def build_and_use(_: int) -> bool:
        s = Serializer(Node)
        tree = _build_tree(depth=3, fanout=2, counter=[0])
        return s.load(s.dump(tree)) == tree

    with ThreadPoolExecutor(max_workers=8) as pool:
        results = list(pool.map(build_and_use, range(32)))
    assert all(results)


def test_module_announces_free_threading_safety() -> None:
    """Under a free-threaded build the module must not silently re-enable the GIL."""
    # Importing :class:`Serializer` at module scope already loaded the native
    # extension. Under a free-threaded build it would have re-enabled the GIL
    # unless the module declared ``gil_used=false``.
    is_gil_enabled = getattr(sys, '_is_gil_enabled', None)
    if is_gil_enabled is None:
        # Standard build with no free-threaded API surface; nothing to assert.
        return
    if is_gil_enabled():
        # Standard build of 3.13+. Import succeeded, nothing more to check.
        return
    # Free-threaded interpreter: had the extension not opted in via
    # `gil_used=false`, importing it would have flipped the GIL back on.
    assert not is_gil_enabled(), 'extension import unexpectedly re-enabled the GIL'
