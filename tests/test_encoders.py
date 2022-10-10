from dataclasses import dataclass
from decimal import Decimal

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
    assert serializer.dump(Decimal(123)) == serializer.load(123) == Decimal(123)


def test_decimal_invalid_value__raise_validation_error():
    serializer = Serializer(Decimal)

    with pytest.raises(ValidationError):
        serializer.load("asd")


def test_dict_encoder():
    serializer = Serializer(dict[str, Decimal])
    val = {
        "a": Decimal(
            "123.3",
        )
    }
    assert serializer.dump(val) == serializer.load({"a": "123.3"}) == val


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
