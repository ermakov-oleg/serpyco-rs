import sys
from dataclasses import dataclass, field

import pytest

from serpyco_rs import Serializer, SchemaValidationError
from typing import List, Optional
from collections.abc import Sequence, Mapping


def test_dump_simple_fields_types():
    @dataclass
    class A:
        int_f: int
        float_f: float
        bool_f: bool
        str_f: str

    serializer = Serializer(A)

    obj = A(
        int_f=123,
        float_f=3.14,
        bool_f=True,
        str_f="Test",
    )
    expected = {"bool_f": True, "float_f": 3.14, "int_f": 123, "str_f": "Test"}
    assert serializer.dump(obj) == expected
    assert serializer.load(expected) == obj


def test_simple_nested_dataclasses():
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

    serializer = Serializer(A)

    obj = A(
        int_f=123,
        nested=B(value="test", nested=C(value=1)),
    )

    expected = {"int_f": 123, "nested": {"nested": {"value": 1}, "value": "test"}}

    assert serializer.dump(obj) == expected
    assert serializer.load(expected) == obj


def test_iterables():
    @dataclass
    class A:
        iterable_builtins_list: list[int]
        iterable_typing_list: List[int]
        iterable_builtins_sequence: Sequence[int]

    serializer = Serializer(A)

    obj = A(
        iterable_builtins_list=[1, 2, 3],
        iterable_typing_list=[1, 2, 3],
        iterable_builtins_sequence=[1, 2, 3],
    )

    expected = {
        "iterable_builtins_list": [1, 2, 3],
        "iterable_typing_list": [1, 2, 3],
        "iterable_builtins_sequence": [1, 2, 3],
    }

    assert serializer.dump(obj) == expected
    assert serializer.load(expected) == obj


def test_mappings():
    @dataclass
    class B:
        value: str

    @dataclass
    class A:
        dict_field: dict[str, int]
        mapping_field: Mapping[str, B]

    serializer = Serializer(A)

    obj = A(dict_field={"foo": 1}, mapping_field={"bar": B(value="123")})

    expected = {
        "dict_field": {"foo": 1},
        "mapping_field": {"bar": {"value": "123"}},
    }

    assert serializer.load(expected) == obj
    assert serializer.dump(obj) == expected


def test_required_and_nullable():
    @dataclass
    class ReqNotNull:
        foo: int

    @dataclass
    class ReqNullable:
        foo: Optional[int]

    @dataclass
    class OptionalNotNull:
        foo: int = 1

    @dataclass
    class OptionalNullable:
        foo: Optional[int] = 1

    req_not_null = Serializer(ReqNotNull)
    req_nullable = Serializer(ReqNullable)
    optional_not_null = Serializer(OptionalNotNull)
    optional_nullable = Serializer(OptionalNullable)

    assert req_not_null.load({"foo": 2}) == ReqNotNull(foo=2)
    with pytest.raises(SchemaValidationError):
        req_not_null.load({"foo": None})
    with pytest.raises(SchemaValidationError):
        req_not_null.load({})

    assert req_nullable.load({"foo": 2}) == ReqNullable(foo=2)
    assert req_nullable.load({"foo": None}) == ReqNullable(foo=None)
    with pytest.raises(SchemaValidationError):
        req_nullable.load({})

    assert optional_not_null.load({"foo": 2}) == OptionalNotNull(foo=2)
    with pytest.raises(SchemaValidationError):
        assert optional_not_null.load({"foo": None})
    assert optional_not_null.load({}) == OptionalNotNull(foo=1)

    assert optional_nullable.load({"foo": 2}) == OptionalNullable(foo=2)
    assert optional_nullable.load({"foo": None}) == OptionalNullable(foo=None)
    assert optional_nullable.load({}) == OptionalNullable(foo=1)


def test_required_and_nullable_list():
    @dataclass
    class Entity:
        foo: Optional[list[Optional[int]]] = None

    entity_serializer = Serializer(Entity)

    assert entity_serializer.load({}) == Entity(foo=None)
    assert entity_serializer.load({"foo": None}) == Entity(foo=None)
    assert entity_serializer.load({"foo": []}) == Entity(foo=[])
    assert entity_serializer.load({"foo": [1]}) == Entity(foo=[1])
    assert entity_serializer.load({"foo": [1, None]}) == Entity(foo=[1, None])


def test_defaults():
    @dataclass
    class Entity:
        foo: str = "123"
        bar: list[int] = field(default_factory=lambda: list([1, 2, 3]))

    entity_serializer = Serializer(Entity)

    assert entity_serializer.load({"bar": [1]}) == Entity(foo="123", bar=[1])
    assert entity_serializer.load({}) == Entity(foo="123", bar=[1, 2, 3])


if sys.version_info >= (3, 10):

    def test_union_optional__dump_load__ok():
        # arrange
        @dataclass
        class UnionClass:
            name: str | None
            count: None | int

        # act
        serializer = Serializer(UnionClass)

        # assert
        foo = UnionClass(name=None, count=None)
        dict_foo = {"name": None, "count": None}
        assert serializer.dump(foo) == dict_foo
        assert foo == serializer.load(dict_foo)

        bar = UnionClass(name="try", count=5)
        dict_bar = {"name": "try", "count": 5}
        assert serializer.dump(bar) == dict_bar
        assert bar == serializer.load(dict_bar)
