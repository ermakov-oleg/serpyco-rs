from dataclasses import dataclass
from typing import Annotated, Any, Optional

import pytest
from serpyco_rs import Serializer
from serpyco_rs.metadata import Alias, Flatten


@pytest.fixture
def ns(request) -> str:
    return f'tests.json_schema.test_flatten.{request.node.name}'


def test_flatten_struct_json_schema(ns):
    @dataclass
    class Address:
        street: str
        city: str
        country: str

    @dataclass
    class Person:
        name: str
        age: int
        address: Annotated[Address, Flatten]

    serializer = Serializer(Person)
    schema = serializer.get_json_schema()

    assert schema == {
        '$ref': f'#/components/schemas/{ns}.<locals>.Person',
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        'components': {
            'schemas': {
                f'{ns}.<locals>.Person': {
                    'properties': {
                        'name': {'type': 'string'},
                        'age': {'type': 'integer', 'format': 'int64'},
                        'street': {'type': 'string'},
                        'city': {'type': 'string'},
                        'country': {'type': 'string'},
                    },
                    'required': ['name', 'age', 'street', 'city', 'country'],
                    'type': 'object',
                },
            },
        },
    }


def test_flatten_dict_json_schema(ns):
    @dataclass
    class PersonWithMeta:
        name: str
        age: int
        metadata: Annotated[dict[str, Any], Flatten]

    serializer = Serializer(PersonWithMeta)
    schema = serializer.get_json_schema()

    assert schema == {
        '$ref': f'#/components/schemas/{ns}.<locals>.PersonWithMeta',
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        'components': {
            'schemas': {
                f'{ns}.<locals>.PersonWithMeta': {
                    'properties': {
                        'name': {'type': 'string'},
                        'age': {'type': 'integer', 'format': 'int64'},
                    },
                    'required': ['name', 'age'],
                    'type': 'object',
                    'additionalProperties': True,
                },
            },
        },
    }


def test_multiple_flatten_structs_json_schema(ns):
    @dataclass
    class ContactInfo:
        email: str
        phone: str

    @dataclass
    class Address:
        street: str
        city: str

    @dataclass
    class PersonWithMultipleFlatten:
        name: str
        contact: Annotated[ContactInfo, Flatten]
        address: Annotated[Address, Flatten]

    serializer = Serializer(PersonWithMultipleFlatten)
    schema = serializer.get_json_schema()

    assert schema == {
        '$ref': f'#/components/schemas/{ns}.<locals>.PersonWithMultipleFlatten',
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        'components': {
            'schemas': {
                f'{ns}.<locals>.PersonWithMultipleFlatten': {
                    'properties': {
                        'name': {'type': 'string'},
                        'email': {'type': 'string'},
                        'phone': {'type': 'string'},
                        'street': {'type': 'string'},
                        'city': {'type': 'string'},
                    },
                    'required': ['name', 'email', 'phone', 'street', 'city'],
                    'type': 'object',
                },
            },
        },
    }


def test_flatten_with_alias_json_schema(ns):
    @dataclass
    class UserData:
        internal_field: Annotated[str, Alias('externalField')]
        value: int

    @dataclass
    class PersonWithAlias:
        name: str
        user_data: Annotated[UserData, Flatten]

    serializer = Serializer(PersonWithAlias)
    schema = serializer.get_json_schema()

    assert schema == {
        '$ref': f'#/components/schemas/{ns}.<locals>.PersonWithAlias',
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        'components': {
            'schemas': {
                f'{ns}.<locals>.PersonWithAlias': {
                    'properties': {
                        'name': {'type': 'string'},
                        'externalField': {'type': 'string'},
                        'value': {'type': 'integer', 'format': 'int64'},
                    },
                    'required': ['name', 'externalField', 'value'],
                    'type': 'object',
                },
            },
        },
    }


def test_flatten_with_camelcase_json_schema(ns):
    @dataclass
    class NestedData:
        long_field_name: str
        another_field: int

    @dataclass
    class PersonWithCamelCase:
        user_name: str
        nested_data: Annotated[NestedData, Flatten]

    serializer = Serializer(PersonWithCamelCase, camelcase_fields=True)
    schema = serializer.get_json_schema()

    assert schema == {
        '$ref': f'#/components/schemas/{ns}.<locals>.PersonWithCamelCase',
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        'components': {
            'schemas': {
                f'{ns}.<locals>.PersonWithCamelCase': {
                    'properties': {
                        'userName': {'type': 'string'},
                        'longFieldName': {'type': 'string'},
                        'anotherField': {'type': 'integer', 'format': 'int64'},
                    },
                    'required': ['userName', 'longFieldName', 'anotherField'],
                    'type': 'object',
                },
            },
        },
    }


def test_nested_flatten_json_schema(ns):
    @dataclass
    class Level3:
        level3_field: str

    @dataclass
    class Level2:
        level2_field: str
        level3: Annotated[Level3, Flatten]

    @dataclass
    class Level1:
        level1_field: str
        level2: Annotated[Level2, Flatten]

    serializer = Serializer(Level1)
    schema = serializer.get_json_schema()

    assert schema == {
        '$ref': f'#/components/schemas/{ns}.<locals>.Level1',
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        'components': {
            'schemas': {
                f'{ns}.<locals>.Level1': {
                    'properties': {
                        'level1_field': {'type': 'string'},
                        'level2_field': {'type': 'string'},
                        'level3_field': {'type': 'string'},
                    },
                    'required': ['level1_field', 'level2_field', 'level3_field'],
                    'type': 'object',
                },
            },
        },
    }


def test_flatten_with_optional_fields_json_schema(ns):
    @dataclass
    class OptionalData:
        required_field: str
        optional_field: Optional[str] = None

    @dataclass
    class PersonWithOptional:
        name: str
        data: Annotated[OptionalData, Flatten]

    serializer = Serializer(PersonWithOptional)
    schema = serializer.get_json_schema()

    assert schema == {
        '$ref': f'#/components/schemas/{ns}.<locals>.PersonWithOptional',
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        'components': {
            'schemas': {
                f'{ns}.<locals>.PersonWithOptional': {
                    'properties': {
                        'name': {'type': 'string'},
                        'required_field': {'type': 'string'},
                        'optional_field': {
                            'anyOf': [
                                {'type': 'null'},
                                {'type': 'string'},
                            ]
                        },
                    },
                    'required': ['name', 'required_field'],
                    'type': 'object',
                },
            },
        },
    }


def test_flatten_dict_with_specific_value_type_json_schema(ns):
    """Test that flatten dict fields include specific value type schema"""

    @dataclass
    class PersonWithTypedMeta:
        name: str
        age: int
        metadata: Annotated[dict[str, int], Flatten]

    serializer = Serializer(PersonWithTypedMeta)
    schema = serializer.get_json_schema()

    assert schema == {
        '$ref': f'#/components/schemas/{ns}.<locals>.PersonWithTypedMeta',
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        'components': {
            'schemas': {
                f'{ns}.<locals>.PersonWithTypedMeta': {
                    'properties': {
                        'name': {'type': 'string'},
                        'age': {'type': 'integer', 'format': 'int64'},
                    },
                    'required': ['name', 'age'],
                    'type': 'object',
                    'additionalProperties': {'type': 'integer', 'format': 'int64'},
                },
            },
        },
    }


def test_flatten_mixed_struct_and_dict_json_schema(ns):
    @dataclass
    class BaseInfo:
        info_field: str

    @dataclass
    class MixedFlatten:
        name: str
        base: Annotated[BaseInfo, Flatten]
        extra: Annotated[dict[str, Any], Flatten]

    serializer = Serializer(MixedFlatten)
    schema = serializer.get_json_schema()

    assert schema == {
        '$ref': f'#/components/schemas/{ns}.<locals>.MixedFlatten',
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        'components': {
            'schemas': {
                f'{ns}.<locals>.MixedFlatten': {
                    'properties': {
                        'name': {'type': 'string'},
                        'info_field': {'type': 'string'},
                    },
                    'required': ['name', 'info_field'],
                    'type': 'object',
                    'additionalProperties': True,
                },
            },
        },
    }
