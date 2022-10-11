import sys
import uuid
from dataclasses import dataclass
from decimal import Decimal
from enum import Enum

import pytest

from serpyco_rs import ValidationError
from serpyco_rs import Serializer


@pytest.mark.parametrize(
    ["type", "value"],
    (
        (int, 44),
        (str, "123"),
        (float, 4.3),
        (bool, True),
    ),
)
def test_simple_types(type, value):
    serializer = Serializer(type)
    assert serializer.load(serializer.dump(value)) == value


def test_decimal():
    serializer = Serializer(Decimal)
    assert serializer.dump(Decimal(123)) == "123"
    assert serializer.load(123) == Decimal(123)
    assert serializer.load("123") == Decimal(123)


def test_decimal_invalid_value__raise_validation_error():
    serializer = Serializer(Decimal)

    with pytest.raises(ValidationError):
        serializer.load("asd")


def test_dict_encoder():
    serializer = Serializer(dict[str, Decimal])
    val = {"a": Decimal("123.3")}
    assert serializer.dump(val) == {"a": "123.3"}
    assert serializer.load({"a": "123.3"}) == val


def test_array_encoder():
    serializer = Serializer(list[int])
    val = [1, 2, 3]
    assert serializer.dump(val) == serializer.load(val) == val


def test_entity_encoder():
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


def test_uuid():
    serializer = Serializer(uuid.UUID)
    val = uuid.uuid4()
    assert serializer.dump(val) == str(val)
    assert serializer.load(str(val)) == val


def test_enum():
    class Foo(Enum):
        foo = "foo"

    serializer = Serializer(Foo)
    assert serializer.dump(Foo.foo) == "foo"
    assert serializer.load("foo") == Foo.foo


if sys.version_info >= (3, 10):

    def test_optional():
        @dataclass
        class T:
            foo: int | None = None

        serializer = Serializer(T)
        assert serializer.dump(T()) == {"foo": None}
        assert serializer.dump(T(foo=1)) == {"foo": 1}
        assert serializer.load({}) == T()
        assert serializer.load({"foo": 12}) == T(foo=12)
