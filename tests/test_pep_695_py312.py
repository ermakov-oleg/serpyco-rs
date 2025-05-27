from dataclasses import dataclass
from typing import Annotated, Literal

import pytest
from serpyco_rs import Serializer
from serpyco_rs.metadata import Discriminator


type Json = str | int | float | bool | None | list[Json] | dict[str, Json]


def test_json_type():
    ser = Serializer(Json)
    assert ser.load(None) == None  # noqa: E711
    assert ser.load(True) == True  # noqa: E712
    assert ser.load(False) == False  # noqa: E712
    assert ser.load(123) == 123
    assert ser.load(123.456) == 123.456
    assert ser.load('hello') == 'hello'
    assert ser.load([1, 2, 3]) == [1, 2, 3]
    assert ser.load({'a': 1, 'b': 2}) == {'a': 1, 'b': 2}


type MyList[T] = list[T]


def test_generic_with_type_param():
    ser = Serializer(MyList[int])
    assert ser.load([1]) == [1]
    assert ser.dump([2]) == [2]


def test_generic_with_unfilled_type_param__fail():
    with pytest.raises(RuntimeError) as exc_info:
        Serializer(MyList)

    assert exc_info.match('Unfilled TypeVar: T')


@dataclass(kw_only=True)
class Foo[T]:
    type: Literal['Foo'] = 'Foo'
    children_field: list[T]


@dataclass(kw_only=True)
class Bar[T]:
    type: Literal['Bar'] = 'Bar'
    children_field: list[T]
    field: int | None = None


type Core = Annotated[Foo[Core] | Bar[Core], Discriminator('type')]


def test_serialize__new_type_var_recursive_generic_dataclass__correct():
    serializer = Serializer(Core, camelcase_fields=True, omit_none=True)

    obj = Bar(
        children_field=[
            Foo(
                children_field=[
                    Bar(
                        children_field=[],
                        field=1,
                    )
                ]
            )
        ]
    )

    data = {
        'childrenField': [
            {
                'childrenField': [
                    {
                        'childrenField': [],
                        'field': 1,
                        'type': 'Bar',
                    }
                ],
                'type': 'Foo',
            }
        ],
        'type': 'Bar',
    }

    assert serializer.dump(obj) == data
    assert serializer.load(data) == obj


def test_multiple_type_params():
    type MyDict[K, V] = dict[K, V]

    ser = Serializer(MyDict[str, int])
    data = {'key1': 1, 'key2': 2}
    assert ser.load(data) == data
    assert ser.dump(data) == data


def test_multiple_type_params_unfilled__fail():
    type MyDict[K, V] = dict[K, V]

    with pytest.raises(RuntimeError) as exc_info:
        Serializer(MyDict)

    assert exc_info.match('Unfilled TypeVar: K')


def test_nested_generics():
    type NestedList[T] = list[list[T]]
    ser = Serializer(NestedList[str])
    data = [['hello', 'world'], ['foo', 'bar']]
    assert ser.load(data) == data
    assert ser.dump(data) == data


def test_union_with_type_params():
    type Result[T, E] = T | E

    ser = Serializer(Result[str, int])
    assert ser.load('success') == 'success'
    assert ser.load(404) == 404
    assert ser.dump('success') == 'success'
    assert ser.dump(404) == 404


def test_complex_nested_structure():
    type ComplexStructure[T] = dict[str, list[T | None]]

    ser = Serializer(ComplexStructure[int])
    data = {'numbers': [1, 2, None, 3], 'more_numbers': [None, 4, 5]}
    assert ser.load(data) == data
    assert ser.dump(data) == data


def test_generic_dataclass_multiple_params():
    @dataclass
    class Container[T, U]:
        first: T
        second: U

    ser = Serializer(Container[str, int])
    obj = Container(first='hello', second=42)
    data = {'first': 'hello', 'second': 42}

    assert ser.dump(obj) == data
    assert ser.load(data) == obj


def test_recursive_type_with_param():
    type Tree[T] = dict[str, T | Tree[T]]
    ser = Serializer(Tree[int])
    data = {'value': 1, 'children': {'left': {'value': 2}, 'right': 3}}
    assert ser.load(data) == data
    assert ser.dump(data) == data
