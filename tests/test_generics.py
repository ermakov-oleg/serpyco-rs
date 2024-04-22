from dataclasses import dataclass
from typing import Generic, Optional, TypeVar
from unittest import mock

from serpyco_rs import Serializer


T = TypeVar('T')


@dataclass
class GenericType(Generic[T]):
    value: Optional[T] = None
    path: Optional[str] = None


@dataclass
class CustomType:
    q: str
    w: int


@dataclass
class A:
    a: GenericType[bool]
    b: GenericType[CustomType]
    c: GenericType[int]
    d: GenericType[float]
    e: GenericType[str]
    f: GenericType[str]
    g: Optional[GenericType[str]] = None


def test_generics__serialization():
    serializer = Serializer(A, omit_none=True)

    a = A(
        a=GenericType(
            value=True,
            path='some_path',
        ),
        b=GenericType(
            value=CustomType(
                q='q',
                w=1,
            ),
            path='some_path',
        ),
        c=GenericType(
            value=1,
            path='some_path',
        ),
        d=GenericType(
            value=2.0,
            path='some_path',
        ),
        e=GenericType(
            value='value',
            path='some_path',
        ),
        f=GenericType(),
        g=None,
    )

    raw_a = {
        'a': {
            'value': True,
            'path': 'some_path',
        },
        'b': {
            'value': {
                'q': 'q',
                'w': 1,
            },
            'path': 'some_path',
        },
        'c': {
            'value': 1,
            'path': 'some_path',
        },
        'd': {
            'value': 2.0,
            'path': 'some_path',
        },
        'e': {
            'value': 'value',
            'path': 'some_path',
        },
        'f': {},
    }

    assert serializer.dump(a) == raw_a
    assert serializer.load(raw_a) == a


def test_generics__swagger_schema():
    serializer = Serializer(A)
    assert serializer.get_json_schema() == {
        '$ref': '#/components/schemas/tests.test_generics.A[no_format,keep_nones]',
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        'components': {
            'schemas': {
                'tests.test_generics.A[no_format,keep_nones]': {
                    'properties': {
                        'a': {
                            '$ref': '#/components/schemas/tests.test_generics.GenericType[bool][no_format,keep_nones]'
                        },
                        'b': {
                            '$ref': '#/components/schemas/tests.test_generics.GenericType[tests.test_generics.CustomType][no_format,keep_nones]'
                        },
                        'c': {
                            '$ref': '#/components/schemas/tests.test_generics.GenericType[int][no_format,keep_nones]'
                        },
                        'd': {
                            '$ref': '#/components/schemas/tests.test_generics.GenericType[float][no_format,keep_nones]'
                        },
                        'e': {
                            '$ref': '#/components/schemas/tests.test_generics.GenericType[str][no_format,keep_nones]'
                        },
                        'f': {
                            '$ref': '#/components/schemas/tests.test_generics.GenericType[str][no_format,keep_nones]'
                        },
                        'g': {
                            'anyOf': [
                                {'type': 'null'},
                                {
                                    '$ref': '#/components/schemas/tests.test_generics.GenericType[str][no_format,keep_nones]'
                                },
                            ]
                        },
                    },
                    'required': ['a', 'b', 'c', 'd', 'e', 'f'],
                    'type': 'object',
                },
                'tests.test_generics.CustomType[no_format,keep_nones]': {
                    'properties': {'q': {'type': 'string'}, 'w': {'type': 'integer'}},
                    'required': ['q', 'w'],
                    'type': 'object',
                },
                'tests.test_generics.GenericType[bool][no_format,keep_nones]': {
                    'properties': {
                        'path': {'anyOf': [{'type': 'null'}, {'type': 'string'}]},
                        'value': {'anyOf': [{'type': 'null'}, {'type': 'boolean'}]},
                    },
                    'type': 'object',
                },
                'tests.test_generics.GenericType[float][no_format,keep_nones]': {
                    'properties': {
                        'path': {'anyOf': [{'type': 'null'}, {'type': 'string'}]},
                        'value': {'anyOf': [{'type': 'null'}, {'type': 'number'}]},
                    },
                    'type': 'object',
                },
                'tests.test_generics.GenericType[int][no_format,keep_nones]': {
                    'properties': {
                        'path': {'anyOf': [{'type': 'null'}, {'type': 'string'}]},
                        'value': {'anyOf': [{'type': 'null'}, {'type': 'integer'}]},
                    },
                    'type': 'object',
                },
                'tests.test_generics.GenericType[str][no_format,keep_nones]': {
                    'properties': {
                        'path': {'anyOf': [{'type': 'null'}, {'type': 'string'}]},
                        'value': {'anyOf': [{'type': 'null'}, {'type': 'string'}]},
                    },
                    'type': 'object',
                },
                'tests.test_generics.GenericType[tests.test_generics.CustomType][no_format,keep_nones]': {
                    'properties': {
                        'path': {'anyOf': [{'type': 'null'}, {'type': 'string'}]},
                        'value': {
                            'anyOf': [
                                {'type': 'null'},
                                {'$ref': '#/components/schemas/tests.test_generics.CustomType[no_format,keep_nones]'},
                            ]
                        },
                    },
                    'type': 'object',
                },
            }
        },
    }


def test_generics_inheritance():
    @dataclass
    class Parent(Generic[T]):
        value: T

    @dataclass
    class Child(Parent[bool]):
        pass

    serializer = Serializer(Child)
    assert serializer.dump(Child(42)) == {'value': 42}
