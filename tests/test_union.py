import sys
from dataclasses import dataclass
from enum import Enum
from typing import Annotated, Literal, Optional, Union

import pytest
from serpyco_rs import ErrorItem, SchemaValidationError, Serializer
from serpyco_rs.metadata import Discriminator


@dataclass
class Foo:
    val: int
    type: Literal['foo'] = 'foo'


@dataclass
class Bar:
    type: Literal['bar']
    val: str


@dataclass
class Buz:
    type: int
    val: str


@dataclass
class Buzz:
    val: str


@dataclass
class Base:
    childs: list[Annotated[Union[Foo, Bar], Discriminator('type')]]


def test_tagged_union():
    serializer = Serializer(Base)
    val = Base(
        childs=[
            Foo(val=1),
            Bar(type='bar', val='12'),
        ]
    )
    raw = {'childs': [{'type': 'foo', 'val': 1}, {'type': 'bar', 'val': '12'}]}
    assert serializer.dump(val) == raw
    assert serializer.load(raw) == val


def test_tagged_union__invalid_discriminator_type():
    @dataclass
    class Inner:
        field: Annotated[Union[Foo, Bar, Buz], Discriminator('type')]

    with pytest.raises(RuntimeError) as exc_info:
        Serializer(Inner)

    assert exc_info.type is RuntimeError
    assert exc_info.value.args[0] == (
        'Type <class \'tests.test_union.Buz\'> has invalid discriminator field "type" with type "<class \'int\'>". '
        'Discriminator supports Literal[<str>], Literal[Enum] with str values.'
    )


def test_tagged_union__union_arg_has_no_discriminator_field():
    @dataclass
    class Inner:
        field: Annotated[Union[Foo, Bar, Buzz], Discriminator('type')]

    with pytest.raises(RuntimeError) as exc_info:
        Serializer(Inner)

    assert exc_info.type is RuntimeError
    assert exc_info.value.args[0] == 'Type <class \'tests.test_union.Buzz\'> does not have discriminator field "type"'


def test_tagged_union__unsupported_types():
    @dataclass
    class Inner:
        field: Annotated[Union[int, str], Discriminator('type')]

    with pytest.raises(RuntimeError) as exc_info:
        Serializer(Inner)

    assert exc_info.type is RuntimeError
    assert exc_info.value.args[0] == (
        'Unions supported only for dataclasses or attrs. Provided: typing.Union[int, str]'
    )


@dataclass
class A:
    type: Literal['A']
    params: int
    children: Optional[list['ComponentsT']] = None


@dataclass
class B:
    type: Literal['B']
    params: str
    children: Optional[list['ComponentsT']] = None


ComponentsT = Annotated[Union[A, B], Discriminator('type')]


@pytest.mark.skipif(sys.version_info < (3, 11), reason='requires python3.11 or higher')
def test_tagged_union__with_forward_refs():
    serializer = Serializer(ComponentsT, omit_none=True)
    data: ComponentsT = A(
        type='A',
        params=123,
        children=[
            A(
                type='A',
                params=1234,
            ),
            B(
                type='B',
                params='foo',
                children=[
                    B(
                        type='B',
                        params='bar',
                    )
                ],
            ),
        ],
    )
    raw_data = {
        'type': 'A',
        'params': 123,
        'children': [
            {'params': 1234, 'type': 'A'},
            {'children': [{'params': 'bar', 'type': 'B'}], 'params': 'foo', 'type': 'B'},
        ],
    }
    assert serializer.dump(data) == raw_data
    assert serializer.load(raw_data) == data


def test_tagged_union__camel_case_filed_format():
    @dataclass
    class A:
        discriminator_with_multiple_words: Literal['A']
        a: int

    @dataclass
    class B:
        discriminator_with_multiple_words: Literal['B']
        b: str

    raw_obj = [
        {
            'discriminatorWithMultipleWords': 'A',
            'a': 128,
        },
        {
            'discriminatorWithMultipleWords': 'B',
            'b': 'foo',
        },
    ]
    obj = [
        A(
            discriminator_with_multiple_words='A',
            a=128,
        ),
        B(
            discriminator_with_multiple_words='B',
            b='foo',
        ),
    ]

    serializer = Serializer(
        list[Annotated[Union[A, B], Discriminator('discriminator_with_multiple_words')]],
        camelcase_fields=True,
    )

    assert serializer.dump(obj) == raw_obj
    assert serializer.load(raw_obj) == obj


def test_tagged_union__enum_tag():
    class Type(Enum):
        A = 'A'
        B = 'B'

    @dataclass
    class A:
        type: Literal[Type.A]
        a: int

    @dataclass
    class B:
        type: Literal[Type.B]
        b: str

    raw_obj = [
        {
            'type': 'A',
            'a': 128,
        },
        {
            'type': 'B',
            'b': 'foo',
        },
    ]
    obj = [
        A(
            type=Type.A,
            a=128,
        ),
        B(
            type=Type.B,
            b='foo',
        ),
    ]

    serializer = Serializer(
        list[Annotated[Union[A, B], Discriminator('type')]],
        camelcase_fields=True,
    )

    assert serializer.dump(obj) == raw_obj
    assert serializer.load(raw_obj) == obj


def test_union_simple_types():
    serializer = Serializer(Union[int, str])
    assert serializer.dump(123) == 123
    assert serializer.load(123) == 123
    assert serializer.dump('123') == '123'
    assert serializer.load('123') == '123'


def test_load_union():
    @dataclass
    class Foo:
        val: int

    @dataclass
    class Bar:
        val: str

    serializer = Serializer(Union[Foo, Bar])

    assert serializer.load({'val': 123}) == Foo(val=123)
    assert serializer.load({'val': '123'}) == Bar(val='123')


def test_load_optional_union():
    @dataclass
    class Foo:
        val: int

    serializer = Serializer(Union[Foo, list[Foo], None])

    assert serializer.load({'val': 123}) == Foo(val=123)
    assert serializer.load([{'val': 1}, {'val': 2}]) == [Foo(val=1), Foo(val=2)]
    assert serializer.load(None) is None


def test_load_union_simple_types__invalid_type():
    serializer = Serializer(Union[int, str])
    with pytest.raises(SchemaValidationError) as exc_info:
        serializer.load(123.0)

    assert exc_info.value.errors == [
        ErrorItem(message='123.0 is not of type "typing.Union[int, str]"', instance_path='')
    ]


def test_tagged_union__python_field_for_keyword():
    """Test discriminator with python_field to handle Python keywords like 'type'."""
    @dataclass
    class FooWithUnderscore:
        type_: Literal['foo']
        value: int

    @dataclass
    class BarWithUnderscore:
        type_: Literal['bar']
        value: str

    serializer = Serializer(
        list[Annotated[Union[FooWithUnderscore, BarWithUnderscore], Discriminator('type', python_field='type_')]]
    )

    obj = [
        FooWithUnderscore(type_='foo', value=1),
        BarWithUnderscore(type_='bar', value='test'),
    ]
    raw = [
        {'type': 'foo', 'value': 1},
        {'type': 'bar', 'value': 'test'},
    ]

    assert serializer.dump(obj) == raw
    assert serializer.load(raw) == obj


def test_tagged_union__python_field_with_camelcase():
    """Test discriminator with python_field and camelcase_fields."""
    @dataclass
    class FooWithUnderscore:
        type_: Literal['foo']
        some_field: int

    @dataclass
    class BarWithUnderscore:
        type_: Literal['bar']
        another_field: str

    serializer = Serializer(
        list[Annotated[Union[FooWithUnderscore, BarWithUnderscore], Discriminator('type', python_field='type_')]],
        camelcase_fields=True,
    )

    obj = [
        FooWithUnderscore(type_='foo', some_field=1),
        BarWithUnderscore(type_='bar', another_field='test'),
    ]
    raw = [
        {'type': 'foo', 'someField': 1},
        {'type': 'bar', 'anotherField': 'test'},
    ]

    assert serializer.dump(obj) == raw
    assert serializer.load(raw) == obj


def test_tagged_union__python_field_different_names():
    """Test discriminator with different Python and JSON field names."""
    @dataclass
    class FooWithKind:
        kind: Literal['foo']
        value: int

    @dataclass
    class BarWithKind:
        kind: Literal['bar']
        value: str

    serializer = Serializer(
        list[Annotated[Union[FooWithKind, BarWithKind], Discriminator('type', python_field='kind')]]
    )

    obj = [
        FooWithKind(kind='foo', value=1),
        BarWithKind(kind='bar', value='test'),
    ]
    raw = [
        {'type': 'foo', 'value': 1},
        {'type': 'bar', 'value': 'test'},
    ]

    assert serializer.dump(obj) == raw
    assert serializer.load(raw) == obj
