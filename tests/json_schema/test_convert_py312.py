from dataclasses import dataclass

from serpyco_rs import Serializer


type Json = None | str | int | float | bool | list[Json] | dict[str, Json]


def test_to_json_schema_json_type():
    ser = Serializer(Json)
    ref_name = 'None | str | int | float | bool | list[Json] | dict[str, Json]'
    assert ser.get_json_schema() == {
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        '$ref': f'#/components/schemas/{ref_name}',
        'components': {
            'schemas': {
                ref_name: {
                    'anyOf': [
                        {'type': 'null'},
                        {'type': 'string'},
                        {'type': 'integer', 'format': 'int64'},
                        {'type': 'number'},
                        {'type': 'boolean'},
                        {'type': 'array', 'items': {'$ref': f'#/components/schemas/{ref_name}'}},
                        {'type': 'object', 'additionalProperties': {'$ref': f'#/components/schemas/{ref_name}'}},
                    ]
                }
            }
        },
    }


def test_to_json_schema__use_ref_for_same_type():
    type StrList = list[str]

    @dataclass
    class Data:
        a: StrList
        b: StrList

    ref = 'tests.json_schema.test_convert_py312.test_to_json_schema__use_ref_for_same_type.<locals>.Data'
    serializer = Serializer(Data)
    assert serializer.get_json_schema() == {
        '$ref': f'#/components/schemas/{ref}',
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        'components': {
            'schemas': {
                ref: {
                    'properties': {
                        'a': {'$ref': '#/components/schemas/list[str]'},
                        'b': {'$ref': '#/components/schemas/list[str]'},
                    },
                    'type': 'object',
                    'required': ['a', 'b'],
                },
                'list[str]': {
                    'items': {'type': 'string'},
                    'type': 'array',
                },
            }
        },
    }
