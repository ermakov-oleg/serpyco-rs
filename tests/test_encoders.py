import sys
import uuid
import json
from dataclasses import dataclass
from datetime import date, datetime, time, timedelta, timezone
from decimal import Decimal
from enum import Enum, IntEnum
from typing import Generic, Literal, TypeVar
from zoneinfo import ZoneInfo

import pytest
from dateutil.tz import tzoffset
from serpyco_rs import Serializer, ValidationError
from typing_extensions import TypedDict


@pytest.mark.parametrize(
    ['type', 'value'],
    (
        (int, 44),
        (str, '123'),
        (float, 4.3),
        (bool, True),
    ),
)
def test_simple_types(type, value):
    serializer = Serializer(type)
    dump_result = serializer.dump(value)
    assert serializer.load(dump_result) == value
    assert serializer.load_json(json.dumps(dump_result)) == value


def test_decimal():
    serializer = Serializer(Decimal)
    assert serializer.dump(Decimal(123)) == '123'
    assert serializer.load(123) == Decimal(123)
    assert serializer.load('123') == Decimal(123)
    assert serializer.load_json('"123"') == Decimal(123)


def test_decimal_invalid_value__raise_validation_error():
    serializer = Serializer(Decimal)

    with pytest.raises(ValidationError):
        serializer.load('asd')

    with pytest.raises(ValidationError):
        serializer.load_json('asd')


def test_dict_encoder():
    serializer = Serializer(dict[str, Decimal])
    val = {'a': Decimal('123.3')}
    assert serializer.dump(val) == {'a': '123.3'}
    assert serializer.load({'a': '123.3'}) == val
    assert serializer.load_json('{"a": "123.3"}') == val


def test_array_encoder():
    serializer = Serializer(list[int])
    val = [1, 2, 3]
    assert serializer.dump(val) == serializer.load(val) == val == serializer.load_json(json.dumps(val))


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
        str_f='Test',
    )
    expected = {'bool_f': True, 'float_f': 3.14, 'int_f': 123, 'str_f': 'Test'}
    assert serializer.dump(obj) == expected
    assert serializer.load(expected) == obj
    assert serializer.load_json(json.dumps(expected)) == obj


def test_uuid():
    serializer = Serializer(uuid.UUID)
    val = uuid.uuid4()
    assert serializer.dump(val) == str(val)
    assert serializer.load(str(val)) == val
    assert serializer.load_json(json.dumps(str(val))) == val


def test_enum():
    class Foo(Enum):
        foo = 'foo'

    serializer = Serializer(Foo)
    assert serializer.dump(Foo.foo) == 'foo'
    assert serializer.load('foo') == Foo.foo
    assert serializer.load_json(json.dumps('foo')) == Foo.foo


def test_tuple():
    serializer = Serializer(tuple[int, bool, str])
    assert serializer.dump((1, True, 's')) == [1, True, 's']
    assert serializer.load([1, True, 's']) == (1, True, 's')
    assert serializer.load_json(json.dumps([1, True, 's'])) == (1, True, 's')


def test_tuple__invalid_number_items():
    serializer = Serializer(tuple[int, bool, str])

    with pytest.raises(ValidationError) as exec_info:
        serializer.dump((1,))
    assert exec_info.value.args[0] == 'Invalid number of items for tuple'

    with pytest.raises(ValidationError) as exec_info:
        serializer.load((1,), validate=False)
    assert exec_info.value.args[0] == 'Invalid number of items for tuple'

    with pytest.raises(ValidationError) as exec_info:
        serializer.load_json('[1]')
    assert exec_info.value.args[0] == '[1] has less than 3 items'


@pytest.mark.parametrize(
    ['value', 'expected'],
    (
        (datetime(2022, 10, 10, 14, 23, 43), '2022-10-10T14:23:43'),
        (datetime(2022, 10, 10, 14, 23, 43, 123456), '2022-10-10T14:23:43.123456'),
        (
            datetime(2022, 10, 10, 14, 23, 43, tzinfo=timezone.utc),
            '2022-10-10T14:23:43+00:00',
        ),
        (
            datetime(2022, 10, 10, 14, 23, 43, tzinfo=timezone(timedelta(hours=1))),
            '2022-10-10T14:23:43+01:00',
        ),
        (
            datetime(2022, 10, 10, 14, 23, 43, tzinfo=ZoneInfo('Europe/Berlin')),
            '2022-10-10T14:23:43+02:00',
        ),
    ),
)
def test_datetime_dump(value, expected):
    serializer = Serializer(datetime)
    assert serializer.dump(value) == expected


@pytest.mark.parametrize(
    ['value', 'expected'],
    (
        ('2022-10-10T14:23:43', datetime(2022, 10, 10, 14, 23, 43)),
        ('2022-10-10T14:23:43.123456', datetime(2022, 10, 10, 14, 23, 43, 123456)),
        (
            '2022-10-10T14:23:43+00:00',
            datetime(2022, 10, 10, 14, 23, 43, tzinfo=timezone.utc),
        ),
        (
            '2022-10-10T14:23:43+01:00',
            datetime(2022, 10, 10, 14, 23, 43, tzinfo=timezone(timedelta(hours=1))),
        ),
        (
            '2022-10-10T14:23:43+02:00',
            datetime(2022, 10, 10, 14, 23, 43, tzinfo=ZoneInfo('Europe/Berlin')),
        ),
    ),
)
def test_datetime_load(value, expected):
    serializer = Serializer(datetime)
    assert serializer.load(value) == expected
    assert serializer.load_json(json.dumps(value)) == expected


@pytest.mark.parametrize(
    ['value', 'expected'],
    [
        ('12:34', time(12, 34)),
        ('12:34:56', time(12, 34, 56)),
        ('12:34:56.000078', time(12, 34, 56, 78)),
        (
            '12:34:56.000078+03:00',
            time(12, 34, 56, 78, tzinfo=tzoffset(None, timedelta(hours=3))),
        ),
    ],
)
def test_time_load(value, expected):
    serializer = Serializer(time)
    assert serializer.load(value) == expected
    assert serializer.load_json(json.dumps(value)) == expected


@pytest.mark.parametrize(
    ['value', 'expected'],
    [
        (time(12, 34), '12:34:00'),
        (time(12, 34, 56), '12:34:56'),
        (time(12, 34, 56, 78), '12:34:56.000078'),
    ],
)
def test_time_dump(value, expected):
    serializer = Serializer(time)
    assert serializer.dump(value) == expected


def test_date():
    serializer = Serializer(date)
    assert serializer.load('2022-10-14') == date(2022, 10, 14)
    assert serializer.load_json(json.dumps('2022-10-13')) == date(2022, 10, 13)
    assert serializer.dump(date(2022, 10, 13)) == '2022-10-13'


def test_literal():
    serializer = Serializer(Literal['foo', 'bar'])
    assert serializer.load('bar') == 'bar'
    assert serializer.load_json('"foo"') == 'foo'
    assert serializer.dump('foo') == 'foo'


@pytest.mark.skipif(sys.version_info < (3, 10), reason='New style unions available after 3.10')
def test_optional():
    @dataclass
    class T:
        foo: int | None = None

    serializer = Serializer(T)
    assert serializer.dump(T()) == {'foo': None}
    assert serializer.dump(T(foo=1)) == {'foo': 1}
    assert serializer.load({}) == T()
    assert serializer.load({'foo': 12}) == T(foo=12)
    assert serializer.load_json("{}") == T()
    assert serializer.load_json('{"foo": 12}') == T(foo=12)


def test_int_enum():
    class Foo(IntEnum):
        foo = 1
        bar = 2

    serializer = Serializer(Foo)
    assert serializer.dump(Foo.foo) == 1
    assert serializer.load(1) == Foo.foo
    assert serializer.load_json('2') == Foo.bar


@pytest.mark.skipif(sys.version_info < (3, 11), reason='StrEnum available after 3.11')
def test_str_enum():
    from enum import StrEnum

    class Foo(StrEnum):
        foo = 'foo'
        bar = 'bar'

    serializer = Serializer(Foo)
    assert serializer.dump(Foo.foo) == 'foo'
    assert serializer.load('bar') == Foo.bar
    assert serializer.load_json('"foo"') == Foo.foo


def test_typed_dict():
    _T = TypeVar('_T')

    class T(TypedDict, Generic[_T]):
        foo_filed: int
        generic_field: _T

    serializer = Serializer(T[bool], camelcase_fields=True)
    assert serializer.dump({'foo_filed': 1, 'generic_field': True}) == {'fooFiled': 1, 'genericField': True}
    assert serializer.load({'fooFiled': 1, 'genericField': True}) == {'foo_filed': 1, 'generic_field': True}
    assert serializer.load_json('{"fooFiled": 1, "genericField": true}') == {'foo_filed': 1, 'generic_field': True}
