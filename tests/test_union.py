import json
import sys
from dataclasses import dataclass
from typing import Annotated, Literal, Optional, Union

import pytest
from serpyco_rs import Serializer
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
        "Type <class 'tests.test_union.Buz'> has invalid discriminator field \"type\" with type \"<class 'int'>\". "
        "Discriminator supports only Literal[<str>]."
    )


def test_tagged_union__union_arg_has_no_discriminator_field():
    @dataclass
    class Inner:
        field: Annotated[Union[Foo, Bar, Buzz], Discriminator('type')]

    with pytest.raises(RuntimeError) as exc_info:
        Serializer(Inner)

    assert exc_info.type is RuntimeError
    assert exc_info.value.args[0] == ("Type <class 'tests.test_union.Buzz'> does not have discriminator field \"type\"")


def test_tagged_union__unsupported_types():
    @dataclass
    class Inner:
        field: Annotated[Union[int, str], Discriminator('type')]

    with pytest.raises(RuntimeError) as exc_info:
        Serializer(Inner)

    assert exc_info.type is RuntimeError
    assert exc_info.value.args[0] == (
        "Unions supported only for dataclasses or attrs. Provided: typing.Union[<class 'int'>,<class 'str'>]"
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
