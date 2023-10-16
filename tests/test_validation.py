import uuid
from dataclasses import dataclass
from datetime import date, datetime, time
from decimal import Decimal
from typing import Annotated, Any

import pytest
from serpyco_rs import SchemaValidationError, Serializer
from serpyco_rs._impl import ErrorItem
from serpyco_rs.metadata import CustomEncoder, Max, MaxLength, Min, MinLength


def _check_errors(s: Serializer, value: Any, expected_errors: list[ErrorItem]):
    with pytest.raises(SchemaValidationError) as json_schema_error:
        s.load(value, validate=True)

    with pytest.raises(SchemaValidationError) as native_schema_error:
        s.load(value, validate=False)

    assert json_schema_error.value.errors == expected_errors
    assert native_schema_error.value.errors == expected_errors


@pytest.mark.parametrize(
    ['value', 'err'],
    [
        (1, '1 is less than the minimum of 10'),
        (101, '101 is greater than the maximum of 100'),
    ],
)
def test_integer_validation(value, err):
    s = Serializer(Annotated[int, Min(10), Max(100)])
    _check_errors(s, value, [ErrorItem(message=err, instance_path='')])


def test_integer_validation__invalid_type():
    s = Serializer(int)
    _check_errors(s, '1', [ErrorItem(message='"1" is not of type "integer"', instance_path='')])


def test_integer__custom_encoder():
    s = Serializer(Annotated[int, CustomEncoder(serialize=lambda x: x + 1, deserialize=lambda x: x - 1)])
    assert s.dump(10) == 11
    assert s.load(10) == 9


@pytest.mark.parametrize(
    ['value', 'err'],
    [
        ('hello', '"hello" is shorter than 6 characters'),
        ('hello world', '"hello world" is longer than 8 characters'),
    ],
)
def test_string_validation(value, err):
    s = Serializer(Annotated[str, MinLength(6), MaxLength(8)])
    _check_errors(s, value, [ErrorItem(message=err, instance_path='')])


def test_string_validation__invalid_type():
    s = Serializer(str)
    _check_errors(s, 1, [ErrorItem(message='1 is not of type "string"', instance_path='')])


@pytest.mark.parametrize(
    ['value', 'err'],
    [
        (1.1, '1.1 is less than the minimum of 10'),
        (101.2, '101.2 is greater than the maximum of 100'),
        (1, '1 is less than the minimum of 10'),
        (101, '101 is greater than the maximum of 100'),
    ],
)
def test_float_validation(value, err):
    s = Serializer(Annotated[float, Min(10.0), Max(100.0)])

    with pytest.raises(SchemaValidationError) as e:
        s.load(value, validate=False)

    assert e.value.errors == [ErrorItem(message=err, instance_path='')]


def test_float_validation__invalid_type():
    s = Serializer(float)
    with pytest.raises(SchemaValidationError) as e:
        s.load('1.1', validate=False)

    assert e.value.errors == [ErrorItem(message='"1.1" is not of type "number"', instance_path='')]


@pytest.mark.parametrize(
    ['value', 'err'],
    [
        ('1.1', '1.1 is less than the minimum of 10'),
        ('101.2', '101.2 is greater than the maximum of 100'),
    ],
)
def test_decimal_validation(value, err):
    s = Serializer(Annotated[Decimal, Min(10), Max(100)])

    with pytest.raises(SchemaValidationError) as e:
        s.load(value, validate=False)

    assert e.value.errors == [ErrorItem(message=err, instance_path='')]


def test_decimal_validation__invalid_type():
    s = Serializer(Decimal)
    with pytest.raises(SchemaValidationError) as e:
        s.load('foo', validate=False)

    assert e.value.errors == [ErrorItem(message='"foo" is not of type "decimal"', instance_path='')]


def test_boolean_validation__invalid_type():
    s = Serializer(bool)
    _check_errors(s, 'foo', [ErrorItem(message='"foo" is not of type "boolean"', instance_path='')])


def test_uuid_validation__invalid_type():
    s = Serializer(uuid.UUID)
    with pytest.raises(SchemaValidationError) as e:
        s.load('foo', validate=False)

    assert e.value.errors == [ErrorItem(message='"foo" is not of type "uuid"', instance_path='')]


def test_time_validation__invalid_type():
    s = Serializer(time)
    with pytest.raises(SchemaValidationError) as e:
        s.load('foo', validate=False)

    assert e.value.errors == [ErrorItem(message='"foo" is not of type "time"', instance_path='')]


def test_datetime_validation__invalid_type():
    s = Serializer(datetime)
    with pytest.raises(SchemaValidationError) as e:
        s.load('foo', validate=False)

    assert e.value.errors == [ErrorItem(message='"foo" is not of type "datetime"', instance_path='')]


def test_date_validation__invalid_type():
    s = Serializer(date)
    with pytest.raises(SchemaValidationError) as e:
        s.load('foo', validate=False)

    assert e.value.errors == [ErrorItem(message='"foo" is not of type "date"', instance_path='')]


def test_dataclass_validation__invalid_type():
    @dataclass
    class A:
        a: int

    s = Serializer(A)
    with pytest.raises(SchemaValidationError) as e:
        s.load('foo', validate=False)

    assert e.value.errors == [ErrorItem(message='"foo" is not of type "object"', instance_path='')]


def test_dataclass_validation__missing_field():
    @dataclass
    class A:
        a: int

    s = Serializer(A)
    _check_errors(s, {}, [ErrorItem(message='"a" is a required property', instance_path='')])


def test_dataclass_validation__missing_field__whith_instance_path():
    @dataclass
    class Foo:
        a: int

    @dataclass
    class Bar:
        foo: Foo

    s = Serializer(Bar)
    _check_errors(s, {'foo': {}}, [ErrorItem(message='"a" is a required property', instance_path='foo')])


