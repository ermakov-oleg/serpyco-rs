from dataclasses import dataclass
from typing import Annotated, Optional

import pytest
from serpyco_rs import Serializer
from serpyco_rs.metadata import JsonSchemaExtension, Max, Min


@pytest.fixture
def ns(request) -> str:
    return f'tests.json_schema.test_json_schema_extension.{request.node.name}'


def test_extension_on_simple_type():
    serializer = Serializer(Annotated[str, JsonSchemaExtension({'x-custom': 'value'})])
    schema = serializer.get_json_schema()
    assert schema['x-custom'] == 'value'
    assert schema['type'] == 'string'


def test_extension_on_dataclass_field(ns):
    @dataclass
    class User:
        email: Annotated[str, JsonSchemaExtension({'x-custom': 'tagged'})]
        name: str

    serializer = Serializer(User)
    schema = serializer.get_json_schema()
    user_schema = schema['components']['schemas'][f'{ns}.<locals>.User']
    assert user_schema['properties']['email']['x-custom'] == 'tagged'
    assert 'x-custom' not in user_schema['properties']['name']


def test_extension_combined_with_other_metadata():
    serializer = Serializer(Annotated[int, Min(0), Max(100), JsonSchemaExtension({'x-audit': True})])
    schema = serializer.get_json_schema()
    assert schema['minimum'] == 0
    assert schema['maximum'] == 100
    assert schema['x-audit'] is True


def test_multiple_extensions_merged():
    serializer = Serializer(Annotated[str, JsonSchemaExtension({'x-a': 1}), JsonSchemaExtension({'x-b': 2})])
    schema = serializer.get_json_schema()
    assert schema['x-a'] == 1
    assert schema['x-b'] == 2


def test_extension_on_optional_field(ns):
    @dataclass
    class Data:
        value: Annotated[Optional[str], JsonSchemaExtension({'x-tag': 'opt'})]

    serializer = Serializer(Data)
    schema = serializer.get_json_schema()
    data_schema = schema['components']['schemas'][f'{ns}.<locals>.Data']
    assert data_schema['properties']['value']['x-tag'] == 'opt'


def test_serialization_unaffected():
    @dataclass
    class Data:
        email: Annotated[str, JsonSchemaExtension({'x-custom': 'value'})]

    serializer = Serializer(Data)
    obj = Data(email='test@example.com')
    dumped = serializer.dump(obj)
    assert dumped == {'email': 'test@example.com'}
    assert serializer.load(dumped) == obj


def test_conflicting_extension_keys_last_wins():
    serializer = Serializer(Annotated[str, JsonSchemaExtension({'x-a': 1}), JsonSchemaExtension({'x-a': 2})])
    schema = serializer.get_json_schema()
    assert schema['x-a'] == 2


def test_extension_on_list():
    serializer = Serializer(Annotated[list[int], JsonSchemaExtension({'x-items': 'ids'})])
    schema = serializer.get_json_schema()
    assert schema['type'] == 'array'
    assert schema['x-items'] == 'ids'
