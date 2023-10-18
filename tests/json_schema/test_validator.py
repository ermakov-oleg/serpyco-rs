import sys
import textwrap
import uuid
from dataclasses import dataclass
from datetime import date, datetime, time
from decimal import Decimal
from enum import Enum
from typing import Annotated, Any, Callable, Literal, Optional, Union
from unittest import mock

import pytest
from serpyco_rs import Serializer
from serpyco_rs.exceptions import ErrorItem, SchemaValidationError, ValidationError
from serpyco_rs.metadata import Discriminator, Max, MaxLength, Min, MinLength
from typing_extensions import NotRequired, Required, TypedDict


class EnumTest(Enum):
    foo = 'foo'
    bar = 'bar'


@dataclass
class EntityTest:
    key: str


@dataclass
class Foo:
    type: Literal['foo']
    val: int


@dataclass
class Bar:
    type: Literal['bar']
    val: str


class TypedDictTotalTrue(TypedDict):
    foo: int
    bar: NotRequired[str]


class TypedDictTotalFalse(TypedDict, total=False):
    foo: int
    bar: Required[str]


@pytest.mark.parametrize(
    ['cls', 'value'],
    (
        (bool, True),
        (bool, False),
        (str, ''),
        (Annotated[str, MinLength(1), MaxLength(3)], '12'),
        (int, -99),
        (Annotated[int, Min(1), Max(1000)], 99),
        (float, 1.3),
        (Annotated[float, Min(0), Max(0.4)], 0.1),
        (Decimal, '0.1'),  # support str
        (Decimal, 0.1),  # or int input
        (Decimal, 'NaN'),  # or int input
        (uuid.UUID, str(uuid.uuid4())),  # support only str input
        (time, '12:34'),
        (time, '12:34:56'),
        (time, '12:34:56.000078'),
        (time, '12:34Z'),
        (time, '12:34+0300'),
        (time, '12:34+03:00'),
        (time, '12:34:00+03:00'),
        (time, '12:34:56.000078+03:00'),
        (time, '12:34:56.000078+00:00'),
        (datetime, '2022-10-10T14:23:43'),
        (datetime, '2022-10-10T14:23:43.123456'),
        (datetime, '2022-10-10T14:23:43.123456Z'),
        (datetime, '2022-10-10T14:23:43.123456+00:00'),
        (datetime, '2022-10-10T14:23:43.123456-00:00'),
        (date, '2020-07-17'),
        (EnumTest, 'foo'),
        (Optional[int], None),
        (Optional[int], 1),
        (EntityTest, {'key': 'val'}),
        (list[int], [1, 2]),
        (dict[str, int], {'a': 1}),
        (tuple[str, int, bool], ['1', 2, True]),
        (Annotated[Union[Foo, Bar], Discriminator('type')], {'type': 'foo', 'val': 1}),
        (Annotated[Union[Foo, Bar], Discriminator('type')], {'type': 'bar', 'val': '1'}),
        (Any, ['1', 2, True]),
        (Any, {}),
        (TypedDictTotalTrue, {'foo': 1}),
        (TypedDictTotalTrue, {'foo': 1, 'bar': '1'}),
        (TypedDictTotalFalse, {'bar': '1'}),
        (TypedDictTotalFalse, {'foo': 1, 'bar': '1'}),
    ),
)
def test_validate(cls, value):
    Serializer(cls).load(value)
    Serializer(cls).load(value, validate=False)


if sys.version_info >= (3, 10):

    @pytest.mark.parametrize(
        ['cls', 'value'],
        (
            (Optional[int], None),
            (int | None, None),
            (int | None, 2),
        ),
    )
    def test_validate_new_union(cls, value):
        Serializer(cls).load(value)
        Serializer(cls).load(value, validate=False)


def _mk_e(m=mock.ANY, ip=mock.ANY) -> Callable[[ErrorItem], None]:
    def cmp(e: ErrorItem) -> None:
        assert e.message == m and e.instance_path == ip

    return cmp


@pytest.mark.parametrize(
    ['cls', 'value', 'check'],
    (
        (bool, 1, _mk_e(m='1 is not of type "boolean"')),
        (str, 1, _mk_e(m='1 is not of type "string"')),
        (
            Annotated[str, MinLength(2)],
            'a',
            _mk_e(m='"a" is shorter than 2 characters'),
        ),
        (
            Annotated[str, MaxLength(2)],
            'aaa',
            _mk_e(m='"aaa" is longer than 2 characters'),
        ),
        (int, 9.1, _mk_e(m='9.1 is not of type "integer"')),
        (int, '9', _mk_e(m='"9" is not of type "integer"')),
        (
            Annotated[int, Min(1)],
            0,
            _mk_e(m='0 is less than the minimum of 1'),
        ),
        (
            Annotated[int, Max(1)],
            10,
            _mk_e(m='10 is greater than the maximum of 1'),
        ),
        (float, None, _mk_e(m='null is not of type "number"')),
        (
            Annotated[float, Min(1.1)],
            0.1,
            _mk_e(m='0.1 is less than the minimum of 1.1'),
        ),
        (
            Annotated[float, Max(1.5)],
            10.1,
            _mk_e(m='10.1 is greater than the maximum of 1.5'),
        ),
        (uuid.UUID, 'asd', _mk_e(ip='')),
        (time, '12:34:a', _mk_e(ip='')),
        (datetime, '2022-10-10//12', _mk_e(ip='')),
        (date, '17-02-2022', _mk_e(ip='')),
        (datetime, '2022-10-10T14:23:43.123456-30:00', _mk_e(ip='')),
        (EnumTest, 'buz', _mk_e(m='"buz" is not one of ["foo","bar"]')),
        (
            Optional[int],
            'foo',
            _mk_e(m='"foo" is not valid under any of the schemas listed in the \'anyOf\' keyword'),
        ),
        (EntityTest, {}, _mk_e(m='"key" is a required property')),
        (
            list[int],
            [1, '1'],
            _mk_e(m='"1" is not of type "integer"', ip='1'),
        ),
        (
            dict[str, int],
            {'a': '1'},
            _mk_e(m='"1" is not of type "integer"', ip='a'),
        ),
        (
            tuple[str, int, bool],
            ['1'],
            _mk_e(m='["1"] has less than 3 items'),
        ),
        (
            tuple[str, int, bool],
            ['1', 1, True, 0],
            _mk_e(m='["1",1,true,0] has more than 3 items'),
        ),
        (
            tuple[str, bool],
            ['1', 1],
            _mk_e(
                m='1 is not of type "boolean"',
                ip='1',
            ),
        ),
        (
            Annotated[Union[Foo, Bar], Discriminator('type')],
            {'type': 'buz'},
            _mk_e(m='{"type":"buz"} is not valid under any of the schemas listed in the \'oneOf\' keyword'),
        ),
        (
            Annotated[Union[Foo, Bar], Discriminator('type')],
            {'type': 'foo', 'val': '123'},
            _mk_e(ip=''),
        ),
        (
            Annotated[Union[Foo, Bar], Discriminator('type')],
            {'type': 'bar', 'val': 1},
            _mk_e(ip=''),
        ),
        (TypedDictTotalTrue, {}, _mk_e(m='"foo" is a required property')),
        (TypedDictTotalFalse, {}, _mk_e(m='"bar" is a required property')),
    ),
)
def test_validate__validation_error(cls, value, check):
    serializer = Serializer(cls)
    with pytest.raises(SchemaValidationError) as exc_info:
        serializer.load(value)

    with pytest.raises(SchemaValidationError) as exc_info_new:
        serializer.load(value, validate=False)

    assert len(exc_info.value.errors) == 1
    assert len(exc_info_new.value.errors) == 1
    check(exc_info.value.errors[0])
    # Some formats differ between jsonschema and serpyco_rs
    error = exc_info_new.value.errors[0]
    error.message = error.message.replace('None', 'null')
    error.message = error.message.replace("['1', 1, True, 0]", '["1",1,true,0]')
    error.message = error.message.replace("['1']", '["1"]')
    check(error)


def test_validate__error_format():
    @dataclass
    class Inner:
        baz: str

    @dataclass
    class A:
        foo: int
        bar: Inner

    serializer = Serializer(A)

    value = {'foo': '1', 'bar': {'buz': None}, 'qux': 0}

    with pytest.raises(SchemaValidationError) as exc_info:
        serializer.load(value)

    assert exc_info.value.errors == [
        ErrorItem(
            message='"baz" is a required property',
            instance_path='bar',
        ),
        ErrorItem(
            message='"1" is not of type "integer"',
            instance_path='foo',
        ),
    ]


def test_validation_exceptions_inheritance():
    serializer = Serializer(int)
    with pytest.raises(SchemaValidationError) as exc_info:
        serializer.load('1')

    assert isinstance(exc_info.value, ValidationError)
    assert exc_info.value.message == 'Schema validation failed'


def test_validation_error_message():
    @dataclass
    class A:
        foo: int
        bar: str

    serializer = Serializer(A)
    with pytest.raises(SchemaValidationError) as exc_info:
        serializer.load({'foo': '1', 'bar': 2})

    error = exc_info.value
    error_item = error.errors[0]

    assert str(error_item) == ("""2 is not of type "string" (instance_path='bar')""")
    assert repr(error_item) == ("""ErrorItem(message='2 is not of type "string"', instance_path='bar')""")
    assert str(error) == textwrap.dedent(
        """\
    Schema validation failed:
    - 2 is not of type "string" (instance_path='bar')
    - "1" is not of type "integer" (instance_path='foo')
      """
    )
    assert repr(error) == textwrap.dedent(
        """\
    SchemaValidationError(
        message="Schema validation failed",
        errors=[
            ErrorItem(message='2 is not of type "string"', instance_path='bar'),
            ErrorItem(message='"1" is not of type "integer"', instance_path='foo'),
        ]
    )"""
    )
