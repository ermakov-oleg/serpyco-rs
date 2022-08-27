from dataclasses import dataclass
from serpyco_rs import make_serializer
from typing import List, Set, Tuple
from collections.abc import Sequence


def test_dump_simple_fields_types():
    @dataclass
    class A:
        int_f: int
        float_f: float
        bool_f: bool
        str_f: str

    serializer = make_serializer(A)

    obj = A(
        int_f=123,
        float_f=3.14,
        bool_f=True,
        str_f='Test',
    )
    expected = {
        'bool_f': True,
        'float_f': 3.14,
        'int_f': 123,
        'str_f': 'Test'
    }
    assert serializer.dump(obj) == expected
    assert serializer.load(expected) == obj


def test_dump_simple_nested_dataclasses():
    @dataclass
    class C:
        value: int

    @dataclass
    class B:
        value: str
        nested: C

    @dataclass
    class A:
        int_f: int
        nested: B

    serializer = make_serializer(A)

    obj = A(
        int_f=123,
        nested=B(
            value='test',
            nested=C(value=1)
        ),
    )

    expected = {
        'int_f': 123,
        'nested': {
            'nested': {'value': 1},
            'value': 'test'}
    }

    assert serializer.dump(obj) == expected
    assert serializer.load(expected) == obj


def test_dump_nested():
    @dataclass
    class C:
        value: int

    @dataclass
    class B:
        value: str
        nested: C

    @dataclass
    class A:
        int_f: int
        nested: B

    serializer = make_serializer(A)

    obj = A(
        int_f=123,
        nested=B(
            value='test',
            nested=C(value=1)
        ),
    )
    expected = {
        'int_f': 123,
        'nested': {
            'nested': {'value': 1},
            'value': 'test'}
    }
    assert serializer.dump(obj) == expected
    assert serializer.load(expected) == obj


def test_union_optional__dump_load__ok():
    # arrange
    @dataclass
    class UnionClass:
        name: str | None
        count: None | int

    # act
    serializer = make_serializer(UnionClass)

    # assert
    foo = UnionClass(name=None, count=None)
    dict_foo = {"name": None, "count": None}
    assert serializer.dump(foo) == dict_foo
    assert foo == serializer.load(dict_foo)

    bar = UnionClass(name='try', count=5)
    dict_bar = {"name": 'try', "count": 5}
    assert serializer.dump(bar) == dict_bar
    assert bar == serializer.load(dict_bar)


def test_dump_iterables():
    @dataclass
    class A:
        iterable_builtins_list: list[int]
        iterable_typing_list: List[int]

        iterable_builtins_set: set[int]
        iterable_typing_set: Set[int]

        iterable_builtins_tuple: tuple[int, ...]
        iterable_typing_tuple: Tuple[int, ...]

        iterable_builtins_sequence: Sequence[int]

    serializer = make_serializer(A)

    obj = A(
        iterable_builtins_list=[1, 2, 3],
        iterable_typing_list=[1, 2, 3],
        iterable_builtins_set={1, 2, 3},
        iterable_typing_set={1, 2, 3},
        iterable_builtins_tuple=(1, 2, 3),
        iterable_typing_tuple=(1, 2, 3),
        iterable_builtins_sequence=[1, 2, 3],
    )

    expected = {
        'iterable_builtins_list': [1, 2, 3],
        'iterable_typing_list': [1, 2, 3],
        'iterable_builtins_set': [1, 2, 3],
        'iterable_typing_set': [1, 2, 3],
        'iterable_builtins_tuple': [1, 2, 3],
        'iterable_typing_tuple': [1, 2, 3],
        'iterable_builtins_sequence': [1, 2, 3],
    }

    assert serializer.dump(obj) == expected
    assert serializer.load(expected) == obj
