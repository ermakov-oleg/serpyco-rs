import sys
import uuid
from dataclasses import dataclass
from datetime import datetime, timezone, timedelta, time, date
from decimal import Decimal
from enum import Enum
from zoneinfo import ZoneInfo

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


def test_tuple():
    serializer = Serializer(tuple[int, bool, str])
    assert serializer.dump((1, True, "s")) == [1, True, "s"]
    assert serializer.load([1, True, "s"]) == (1, True, "s")


def test_tuple__invalid_number_items():
    serializer = Serializer(tuple[int, bool, str])

    with pytest.raises(ValidationError) as exec_info:
        serializer.dump((1,))
    assert exec_info.value.args[0] == "Invalid number of items for tuple"

    with pytest.raises(ValidationError) as exec_info:
        serializer.load((1,), validate=False)
    assert exec_info.value.args[0] == "Invalid number of items for tuple"


@pytest.mark.parametrize(
    ["value", "expected"],
    (
        (datetime(2022, 10, 10, 14, 23, 43), "2022-10-10T14:23:43"),
        (datetime(2022, 10, 10, 14, 23, 43, 123456), "2022-10-10T14:23:43.123456"),
        (
            datetime(2022, 10, 10, 14, 23, 43, tzinfo=timezone.utc),
            "2022-10-10T14:23:43+00:00",
        ),
        (
            datetime(2022, 10, 10, 14, 23, 43, tzinfo=timezone(timedelta(hours=1))),
            "2022-10-10T14:23:43+01:00",
        ),
        (
            datetime(2022, 10, 10, 14, 23, 43, tzinfo=ZoneInfo("Europe/Berlin")),
            "2022-10-10T14:23:43+02:00",
        ),
    ),
)
def test_datetime_dump(value, expected):
    serializer = Serializer(datetime)
    assert serializer.dump(value) == expected


@pytest.mark.parametrize(
    ["value", "expected"],
    (
        # ('2022-10-10T14:23:43', datetime(2022, 10, 10, 14, 23, 43)),
        # ('2022-10-10T14:23:43.123456', datetime(2022, 10, 10, 14, 23, 43, 123456)),
        (
            "2022-10-10T14:23:43+00:00",
            datetime(2022, 10, 10, 14, 23, 43, tzinfo=timezone.utc),
        ),
        (
            "2022-10-10T14:23:43+01:00",
            datetime(2022, 10, 10, 14, 23, 43, tzinfo=timezone(timedelta(hours=1))),
        ),
        (
            "2022-10-10T14:23:43+02:00",
            datetime(2022, 10, 10, 14, 23, 43, tzinfo=ZoneInfo("Europe/Berlin")),
        ),
    ),
)
def test_datetime_load(value, expected):
    serializer = Serializer(datetime)
    assert serializer.load(value) == expected


@pytest.mark.parametrize(
    ["value", "expected"],
    [
        ("12:34", time(12, 34)),
        ("12:34:56", time(12, 34, 56)),
        ("12:34:56.000078", time(12, 34, 56, 78)),
        # ('12:34:56.000078+00:00', time(12, 34, 56, 78, tzinfo=tzoffset(None, 0))),
    ],
)
def test_time_load(value, expected):
    serializer = Serializer(time)
    assert serializer.load(value) == expected


@pytest.mark.parametrize(
    ["value", "expected"],
    [
        (time(12, 34), "12:34:00"),
        (time(12, 34, 56), "12:34:56"),
        (time(12, 34, 56, 78), "12:34:56.000078"),
    ],
)
def test_time_dump(value, expected):
    serializer = Serializer(time)
    assert serializer.dump(value) == expected


def test_date():
    serializer = Serializer(date)
    assert serializer.load("2022-10-14") == date(2022, 10, 14)
    assert serializer.dump(date(2022, 10, 13)) == "2022-10-13"


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
