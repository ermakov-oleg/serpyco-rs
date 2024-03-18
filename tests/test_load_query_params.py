"""Query params in most cases, field values are of string type"""

from dataclasses import dataclass
from typing import Annotated, Literal, TypedDict, Union
from enum import Enum

import pytest
from multidict import MultiDict

import serpyco_rs
from serpyco_rs import metadata


def test_load_query_params__flat_type__exception():
    serializer = serpyco_rs.Serializer(int)
    with pytest.raises(RuntimeError) as exc_info:
        serializer.load_query_params(MultiDict({'id': 'a'}))

    assert str(exc_info.value) == 'This type is not deserializable from query params'


def test_load_query_params__int():
    @dataclass
    class Foo:
        id: int

    serializer = serpyco_rs.Serializer(Foo)
    assert serializer.load_query_params(MultiDict({'id': '1'})) == Foo(id=1)


def test_load_query_params__int__empty():
    @dataclass
    class Foo:
        id: int

    serializer = serpyco_rs.Serializer(Foo)
    with pytest.raises(serpyco_rs.SchemaValidationError):
        serializer.load_query_params(MultiDict())


def test_load_query_params__int__invalid():
    @dataclass
    class Foo:
        id: int

    serializer = serpyco_rs.Serializer(Foo)
    with pytest.raises(serpyco_rs.SchemaValidationError):
        serializer.load_query_params(MultiDict({'id': '1.1'}))


def test_load_query_params__float():
    @dataclass
    class Foo:
        id: float

    serializer = serpyco_rs.Serializer(Foo)
    assert serializer.load_query_params(MultiDict({'id': '1.1'})) == Foo(id=1.1)


def test_load_query_params__float__invalid():
    @dataclass
    class Foo:
        id: float

    serializer = serpyco_rs.Serializer(Foo)
    with pytest.raises(serpyco_rs.SchemaValidationError):
        serializer.load_query_params(MultiDict({'id': '1.1.1'}))


def test_load_query_params__bool():
    @dataclass
    class Foo:
        f: bool

    serializer = serpyco_rs.Serializer(Foo)
    assert serializer.load_query_params(MultiDict({'f': 't'})) == Foo(f=True)
    assert serializer.load_query_params(MultiDict({'f': 'T'})) == Foo(f=True)
    assert serializer.load_query_params(MultiDict({'f': 'true'})) == Foo(f=True)
    assert serializer.load_query_params(MultiDict({'f': 'True'})) == Foo(f=True)
    assert serializer.load_query_params(MultiDict({'f': 'TRUE'})) == Foo(f=True)

    assert serializer.load_query_params(MultiDict({'f': 'f'})) == Foo(f=False)
    assert serializer.load_query_params(MultiDict({'f': 'F'})) == Foo(f=False)
    assert serializer.load_query_params(MultiDict({'f': 'false'})) == Foo(f=False)
    assert serializer.load_query_params(MultiDict({'f': 'False'})) == Foo(f=False)
    assert serializer.load_query_params(MultiDict({'f': 'FALSE'})) == Foo(f=False)


def test_load_query_params__bool__invalid():
    @dataclass
    class Foo:
        f: bool

    serializer = serpyco_rs.Serializer(Foo)
    with pytest.raises(serpyco_rs.SchemaValidationError):
        serializer.load_query_params(MultiDict({'f': '1'}))


def test_load_query_params__list():
    @dataclass
    class Foo:
        ids: list[int]

    serializer = serpyco_rs.Serializer(Foo)
    data = MultiDict([('ids', '1'), ('ids', 2), ('ids', '3')])
    assert serializer.load_query_params(data) == Foo(ids=[1, 2, 3])


def test_load_query_params__list__invalid():
    @dataclass
    class Foo:
        ids: list[int]

    serializer = serpyco_rs.Serializer(Foo)
    with pytest.raises(serpyco_rs.SchemaValidationError):
        serializer.load_query_params(MultiDict([('ids', 1), ('ids', '2.12'), ('ids', 3)]))


def test_load_query_params__list__empty():
    @dataclass
    class Foo:
        ids: list[int]

    serializer = serpyco_rs.Serializer(Foo)
    with pytest.raises(serpyco_rs.SchemaValidationError):
        serializer.load_query_params(MultiDict())


def test_load_query_params__list__optional():
    @dataclass
    class Foo:
        ids: Union[list[int], None] = None

    serializer = serpyco_rs.Serializer(Foo)
    assert serializer.load_query_params(MultiDict()) == Foo(ids=None)


def test_load_query_params__tuple():
    @dataclass
    class Foo:
        ids: tuple[int, bool]

    serializer = serpyco_rs.Serializer(Foo)
    assert serializer.load_query_params(MultiDict([('ids', '1'), ('ids', 'true')])) == Foo(ids=(1, True))


def test_load_query_params__tuple__invalid():
    @dataclass
    class Foo:
        ids: tuple[int, bool]

    serializer = serpyco_rs.Serializer(Foo)
    with pytest.raises(serpyco_rs.SchemaValidationError):
        serializer.load_query_params(MultiDict([('ids', '1')]))


def test_load_query_params__union():
    @dataclass
    class Foo:
        f: Union[int, bool]

    serializer = serpyco_rs.Serializer(Foo)
    assert serializer.load_query_params(MultiDict({'f': '1'})) == Foo(f=1)
    assert serializer.load_query_params(MultiDict({'f': 'true'})) == Foo(f=True)


def test_load_query_params__custom_encoder():
    @dataclass
    class Foo:
        id: Annotated[int, metadata.deserialize_with(lambda value: int(value.split('-')[1]))]

    serializer = serpyco_rs.Serializer(Foo)
    assert serializer.load_query_params(MultiDict({'id': '1-2'})) == Foo(id=2)


def test_load_query_params__custom_encoder_list():
    @dataclass
    class Foo:
        id: Annotated[list[int], metadata.deserialize_with(lambda value: [int(v.split('-')[1]) for v in value])]

    serializer = serpyco_rs.Serializer(Foo)
    assert serializer.load_query_params(MultiDict([('id', '1-2'), ('id', '3-4')])) == Foo(id=[2, 4])


class FooEnum(Enum):
    A = 'a'
    B = 2


def test_load_query_params__enum():

    @dataclass
    class Foo:
        e: FooEnum

    serializer = serpyco_rs.Serializer(Foo)
    assert serializer.load_query_params(MultiDict({'e': 'a'})) == Foo(e=FooEnum.A)
    assert serializer.load_query_params(MultiDict({'e': '2'})) == Foo(e=FooEnum.B)


def test_load_query_params__literal():

    @dataclass
    class Foo:
        e: Literal['a', 2]

    serializer = serpyco_rs.Serializer(Foo)
    assert serializer.load_query_params(MultiDict({'e': 'a'})) == Foo(e='a')
    assert serializer.load_query_params(MultiDict({'e': '2'})) == Foo(e=2)


def test_load_query_params__typed_dict():

    class Foo(TypedDict):
        id: int
        name: str

    serializer = serpyco_rs.Serializer(Foo)
    assert serializer.load_query_params(MultiDict({'id': '1', 'name': 'test'})) == Foo(id=1, name='test')


def test_load_query_params__dict():
    serializer = serpyco_rs.Serializer(dict[str, int])
    assert serializer.load_query_params(MultiDict({'id': '1'})) == {'id': 1}


def test_load_query_params__dict_of_lists():
    serializer = serpyco_rs.Serializer(dict[str, list[int]])
    assert serializer.load_query_params(MultiDict([('foo', '1'), ('foo', '2'), ('bar', '3')])) == {
        'foo': [1, 2],
        'bar': [3],
    }
