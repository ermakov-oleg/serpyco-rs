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
                f'{ref_name}': {
                    'oneOf': [
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
