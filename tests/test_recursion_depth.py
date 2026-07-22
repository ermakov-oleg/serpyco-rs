"""Recursion-depth guard converts cycles and excessive nesting into a
`RecursionError` instead of overflowing the Rust thread stack.

Tests pin an explicit low ``max_recursion_depth`` so they exercise the guard
without depending on the platform's actual stack size.
"""

from dataclasses import dataclass
from typing import Optional

import pytest

from serpyco_rs import Serializer


@dataclass
class Node:
    label: str
    next: Optional['Node'] = None


def test_dump_self_cycle_raises_recursion_error():
    s = Serializer(Node, max_recursion_depth=100)
    a = Node('a')
    a.next = a
    with pytest.raises(RecursionError):
        s.dump(a)


def test_dump_two_node_cycle_raises_recursion_error():
    s = Serializer(Node, max_recursion_depth=100)
    a = Node('a')
    b = Node('b')
    a.next = b
    b.next = a
    with pytest.raises(RecursionError):
        s.dump(a)


def test_dump_deep_chain_raises_recursion_error():
    root = Node('0')
    cur = root
    for i in range(1, 500):
        cur.next = Node(str(i))
        cur = cur.next

    s = Serializer(Node, max_recursion_depth=100)
    with pytest.raises(RecursionError):
        s.dump(root)


def test_load_deep_chain_raises_recursion_error():
    raw: Optional[dict] = None
    for i in range(500 - 1, -1, -1):
        raw = {'label': str(i), 'next': raw}

    s = Serializer(Node, max_recursion_depth=100)
    with pytest.raises(RecursionError):
        s.load(raw)


def test_shallow_nesting_still_works():
    root = Node('0')
    cur = root
    for i in range(1, 100):
        cur.next = Node(str(i))
        cur = cur.next

    s = Serializer(Node)
    out = s.dump(root)
    depth = 0
    c = out
    while isinstance(c.get('next'), dict):
        depth += 1
        c = c['next']
    assert depth == 99


def test_custom_max_recursion_depth_can_be_raised():
    root = Node('0')
    cur = root
    for i in range(1, 300):
        cur.next = Node(str(i))
        cur = cur.next

    s = Serializer(Node, max_recursion_depth=2000)
    out = s.dump(root)
    depth = 0
    c = out
    while isinstance(c.get('next'), dict):
        depth += 1
        c = c['next']
    assert depth == 299
