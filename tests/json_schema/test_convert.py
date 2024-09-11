import sys
from dataclasses import dataclass
from datetime import datetime, time
from decimal import Decimal
from enum import Enum
from typing import Annotated, Any, Literal, Optional, TypedDict, Union
from uuid import UUID

import pytest
from serpyco_rs import Serializer, JsonSchemaBuilder
from serpyco_rs.metadata import Alias, CamelCase, Discriminator, Max, MaxLength, Min, MinLength, OmitNone


def test_to_json_schema():
    class EnumCls(Enum):
        a = 'a'

    @dataclass
    class InnerData:
        """Some important entity"""

        foo_filed: str

    @dataclass
    class OtherInnerData:
        """OtherInnerData"""

        optional_filed: Optional[InnerData] = None

    class AnotherInnerData(TypedDict):
        """AnotherInnerData"""

        bar: str

    @dataclass
    class Data:
        """Docs"""

        a: Annotated[int, Min(0), Max(10)]
        """A field with bounds"""
        b: float
        c: Decimal
        d: bool
        e: Annotated[str, MinLength(1), MaxLength(5)]
        f: UUID
        g: time
        h: datetime
        i: EnumCls
        j: InnerData
        k: list[int]
        l: tuple[int, str, InnerData]
        m: dict[str, int]
        n: Any
        o: Annotated[InnerData, CamelCase]
        p: Annotated[OtherInnerData, OmitNone]
        q: AnotherInnerData
        some_filed: Annotated[str, Alias('fooFiled')]

    serializer = Serializer(Data)

    assert serializer.get_json_schema() == {
        '$ref': '#/components/schemas/tests.json_schema.test_convert.test_to_json_schema.<locals>.Data',
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        'components': {
            'schemas': {
                'tests.json_schema.test_convert.test_to_json_schema.<locals>.Data': {
                    'description': 'Docs',
                    'properties': {
                        'a': {
                            'description': 'A field with bounds',
                            'maximum': 10,
                            'minimum': 0,
                            'type': 'integer',
                        },
                        'b': {'type': 'number'},
                        'c': {
                            'oneOf': [{'type': 'string', 'format': 'decimal'}, {'type': 'number', 'format': 'decimal'}]
                        },
                        'd': {'type': 'boolean'},
                        'e': {'maxLength': 5, 'minLength': 1, 'type': 'string'},
                        'f': {'format': 'uuid', 'type': 'string'},
                        'g': {
                            'format': 'time',
                            'type': 'string',
                        },
                        'h': {
                            'format': 'date-time',
                            'type': 'string',
                        },
                        'i': {'enum': ['a'], 'type': 'string'},
                        'j': {
                            '$ref': '#/components/schemas/tests.json_schema.test_convert.test_to_json_schema.<locals>.InnerData'
                        },
                        'k': {'items': {'type': 'integer'}, 'type': 'array'},
                        'l': {
                            'maxItems': 3,
                            'minItems': 3,
                            'prefixItems': [
                                {'type': 'integer'},
                                {'type': 'string'},
                                {
                                    '$ref': '#/components/schemas/tests.json_schema.test_convert.test_to_json_schema.<locals>.InnerData'
                                },
                            ],
                            'type': 'array',
                        },
                        'm': {'additionalProperties': {'type': 'integer'}, 'type': 'object'},
                        'n': {},
                        'p': {
                            '$ref': '#/components/schemas/tests.json_schema.test_convert.test_to_json_schema.<locals>.OtherInnerData'
                        },
                        'o': {
                            '$ref': '#/components/schemas/tests.json_schema.test_convert.test_to_json_schema.<locals>.InnerData1'
                        },
                        'q': {
                            '$ref': '#/components/schemas/tests.json_schema.test_convert.test_to_json_schema.<locals>.AnotherInnerData'
                        },
                        'fooFiled': {'type': 'string'},
                    },
                    'required': [
                        'a',
                        'b',
                        'c',
                        'd',
                        'e',
                        'f',
                        'g',
                        'h',
                        'i',
                        'j',
                        'k',
                        'l',
                        'm',
                        'n',
                        'o',
                        'p',
                        'q',
                        'fooFiled',
                    ],
                    'type': 'object',
                },
                'tests.json_schema.test_convert.test_to_json_schema.<locals>.InnerData': {
                    'description': 'Some important entity',
                    'properties': {'foo_filed': {'type': 'string'}},
                    'required': ['foo_filed'],
                    'type': 'object',
                },
                'tests.json_schema.test_convert.test_to_json_schema.<locals>.InnerData2': {
                    'description': 'Some important entity',
                    'properties': {'foo_filed': {'type': 'string'}},
                    'required': ['foo_filed'],
                    'type': 'object',
                },
                'tests.json_schema.test_convert.test_to_json_schema.<locals>.InnerData1': {
                    'description': 'Some important entity',
                    'properties': {'fooFiled': {'type': 'string'}},
                    'required': ['fooFiled'],
                    'type': 'object',
                },
                'tests.json_schema.test_convert.test_to_json_schema.<locals>.OtherInnerData': {
                    'description': 'OtherInnerData',
                    'properties': {
                        'optional_filed': {
                            'anyOf': [
                                {'type': 'null'},
                                {
                                    '$ref': '#/components/schemas/tests.json_schema.test_convert.test_to_json_schema.<locals>.InnerData2'
                                },
                            ]
                        }
                    },
                    'type': 'object',
                },
                'tests.json_schema.test_convert.test_to_json_schema.<locals>.AnotherInnerData': {
                    'description': 'AnotherInnerData',
                    'properties': {'bar': {'type': 'string'}},
                    'required': ['bar'],
                    'type': 'object',
                },
            },
        },
    }


@pytest.mark.skipif(sys.version_info < (3, 10), reason='New style unions available after 3.10')
def test_to_json_schema__new_union_syntax():
    @dataclass
    class Data:
        """Docs"""

        a: int | None

    serializer = Serializer(Data)

    assert serializer.get_json_schema() == {
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        '$ref': '#/components/schemas/tests.json_schema.test_convert.test_to_json_schema__new_union_syntax.<locals>.Data',
        'components': {
            'schemas': {
                'tests.json_schema.test_convert.test_to_json_schema__new_union_syntax.<locals>.Data': {
                    'description': 'Docs',
                    'properties': {'a': {'anyOf': [{'type': 'null'}, {'type': 'integer'}]}},
                    'required': ['a'],
                    'type': 'object',
                }
            },
        },
    }


@dataclass
class TreeNode:
    """Node"""

    data: str
    left: Optional['TreeNode'] = None
    right: Optional['TreeNode'] = None


def test_to_json_schema__recursive_type():
    serializer = Serializer(TreeNode)

    assert serializer.get_json_schema() == {
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        '$ref': '#/components/schemas/tests.json_schema.test_convert.TreeNode',
        'components': {
            'schemas': {
                'tests.json_schema.test_convert.TreeNode': {
                    'description': 'Node',
                    'properties': {
                        'data': {'type': 'string'},
                        'left': {
                            'anyOf': [
                                {'type': 'null'},
                                {'$ref': '#/components/schemas/tests.json_schema.test_convert.TreeNode'},
                            ]
                        },
                        'right': {
                            'anyOf': [
                                {'type': 'null'},
                                {'$ref': '#/components/schemas/tests.json_schema.test_convert.TreeNode'},
                            ]
                        },
                    },
                    'required': ['data'],
                    'type': 'object',
                }
            }
        },
    }


def test_to_json_schema__literal():
    class BarEnum(Enum):
        bar = 'bar'

    serializer = Serializer(Literal['foo', 1, BarEnum.bar])
    assert serializer.get_json_schema() == {
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        'enum': ['foo', 1, 'bar'],
    }


def test_to_json_schema__tagged_union():
    @dataclass
    class Foo:
        val: int
        type: Literal['foo']

    @dataclass
    class Bar:
        val: str
        type: Literal['bar'] = 'bar'

    class BuzEnum(Enum):
        buz = 'buz'

    @dataclass
    class Baz:
        val: float
        type: Literal[BuzEnum.buz] = BuzEnum.buz

    @dataclass
    class Base:
        field: Annotated[Union[Foo, Bar, Baz], Discriminator('type')]

    serializer = Serializer(Base)
    assert serializer.get_json_schema() == {
        '$ref': '#/components/schemas/tests.json_schema.test_convert.test_to_json_schema__tagged_union.<locals>.Base',
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        'components': {
            'schemas': {
                'tests.json_schema.test_convert.test_to_json_schema__tagged_union.<locals>.Bar': {
                    'properties': {'type': {'enum': ['bar']}, 'val': {'type': 'string'}},
                    'required': ['val', 'type'],
                    'type': 'object',
                },
                'tests.json_schema.test_convert.test_to_json_schema__tagged_union.<locals>.Base': {
                    'properties': {
                        'field': {
                            'discriminator': {
                                'mapping': {
                                    'bar': '#/components/schemas/tests.json_schema.test_convert.test_to_json_schema__tagged_union.<locals>.Bar',
                                    'buz': '#/components/schemas/tests.json_schema.test_convert.test_to_json_schema__tagged_union.<locals>.Baz',
                                    'foo': '#/components/schemas/tests.json_schema.test_convert.test_to_json_schema__tagged_union.<locals>.Foo',
                                },
                                'propertyName': 'type',
                            },
                            'oneOf': [
                                {
                                    '$ref': '#/components/schemas/tests.json_schema.test_convert.test_to_json_schema__tagged_union.<locals>.Foo'
                                },
                                {
                                    '$ref': '#/components/schemas/tests.json_schema.test_convert.test_to_json_schema__tagged_union.<locals>.Bar'
                                },
                                {
                                    '$ref': '#/components/schemas/tests.json_schema.test_convert.test_to_json_schema__tagged_union.<locals>.Baz'
                                },
                            ],
                        }
                    },
                    'required': ['field'],
                    'type': 'object',
                },
                'tests.json_schema.test_convert.test_to_json_schema__tagged_union.<locals>.Baz': {
                    'properties': {'type': {'enum': ['buz']}, 'val': {'type': 'number'}},
                    'required': ['val', 'type'],
                    'type': 'object',
                },
                'tests.json_schema.test_convert.test_to_json_schema__tagged_union.<locals>.Foo': {
                    'properties': {'type': {'enum': ['foo']}, 'val': {'type': 'integer'}},
                    'required': ['val', 'type'],
                    'type': 'object',
                },
            }
        },
    }


def test_to_json_schema__union():
    @dataclass
    class Foo:
        val: int

    @dataclass
    class Bar:
        val: str

    @dataclass
    class Base:
        field: Union[Foo, Bar, str]

    serializer = Serializer(Base)
    assert serializer.get_json_schema() == {
        '$ref': '#/components/schemas/tests.json_schema.test_convert.test_to_json_schema__union.<locals>.Base',
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        'components': {
            'schemas': {
                'tests.json_schema.test_convert.test_to_json_schema__union.<locals>.Bar': {
                    'properties': {'val': {'type': 'string'}},
                    'required': ['val'],
                    'type': 'object',
                },
                'tests.json_schema.test_convert.test_to_json_schema__union.<locals>.Base': {
                    'properties': {
                        'field': {
                            'oneOf': [
                                {
                                    '$ref': '#/components/schemas/tests.json_schema.test_convert.test_to_json_schema__union.<locals>.Foo'
                                },
                                {
                                    '$ref': '#/components/schemas/tests.json_schema.test_convert.test_to_json_schema__union.<locals>.Bar'
                                },
                                {'type': 'string'},
                            ],
                        }
                    },
                    'required': ['field'],
                    'type': 'object',
                },
                'tests.json_schema.test_convert.test_to_json_schema__union.<locals>.Foo': {
                    'properties': {'val': {'type': 'integer'}},
                    'required': ['val'],
                    'type': 'object',
                },
            }
        },
    }


def test_to_json_schema__force_none_as_default_for_optional():
    @dataclass
    class Data:
        a: Optional[int]

    serializer = Serializer(Data, force_default_for_optional=True)
    assert serializer.get_json_schema() == {
        '$ref': '#/components/schemas/tests.json_schema.test_convert.test_to_json_schema__force_none_as_default_for_optional.<locals>.Data',
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        'components': {
            'schemas': {
                'tests.json_schema.test_convert.test_to_json_schema__force_none_as_default_for_optional.<locals>.Data': {
                    'properties': {'a': {'anyOf': [{'type': 'null'}, {'type': 'integer'}]}},
                    'type': 'object',
                }
            }
        },
    }


def test_to_json_schema__bytes():
    @dataclass
    class Data:
        a: bytes

    serializer = Serializer(Data)
    assert serializer.get_json_schema() == {
        '$ref': '#/components/schemas/tests.json_schema.test_convert.test_to_json_schema__bytes.<locals>.Data',
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        'components': {
            'schemas': {
                'tests.json_schema.test_convert.test_to_json_schema__bytes.<locals>.Data': {
                    'properties': {'a': {'type': 'string', 'format': 'binary'}},
                    'type': 'object',
                    'required': ['a'],
                }
            }
        },
    }


def test_enum_x_attrs():
    class EnumCls(Enum):
        a = 'a'
        """a docstring"""

    serializer = Serializer(EnumCls)
    assert serializer.get_json_schema() == {
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        'enum': ['a'],
        'type': 'string',
        'x-a': 'a docstring',
    }


def test_enum_multiply_types():
    class EnumCls(Enum):
        a = 'a'
        """a docstring"""
        b = 1
        """b docstring"""

    serializer = Serializer(EnumCls)
    assert serializer.get_json_schema() == {
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        'enum': ['a', 1],
        'x-a': 'a docstring',
        'x-1': 'b docstring',
    }


def test_array_length():
    serializer = Serializer(Annotated[list[int], MinLength(1), MaxLength(10)])
    assert serializer.get_json_schema() == {
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        'type': 'array',
        'items': {'type': 'integer'},
        'minItems': 1,
        'maxItems': 10,
    }


def test_one_dataclass_with_different_annotations__should_generate_different_schemas():
    @dataclass
    class Foo:
        filed_name: str

    @dataclass
    class Bar:
        foo1: Foo
        foo2: Annotated[Foo, CamelCase]

    schema = Serializer(Bar)
    name_prefix = (
        'tests.json_schema.test_convert.'
        'test_one_dataclass_with_different_annotations__should_generate_different_schemas.<locals>'
    )
    assert schema.get_json_schema() == {
        '$ref': f'#/components/schemas/{name_prefix}.Bar',
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        'components': {
            'schemas': {
                f'{name_prefix}.Bar': {
                    'properties': {
                        'foo1': {'$ref': f'#/components/schemas/{name_prefix}.Foo'},
                        'foo2': {'$ref': f'#/components/schemas/{name_prefix}.Foo1'},
                    },
                    'required': ['foo1', 'foo2'],
                    'type': 'object',
                },
                f'{name_prefix}.Foo': {
                    'properties': {'filed_name': {'type': 'string'}},
                    'required': ['filed_name'],
                    'type': 'object',
                },
                f'{name_prefix}.Foo1': {
                    'properties': {'filedName': {'type': 'string'}},
                    'required': ['filedName'],
                    'type': 'object',
                },
            }
        },
    }


class TestJsonSchemaBuilder:
    def test_build__use_custom_ref_prefix(self):
        @dataclass
        class Data:
            a: int

        serializer = Serializer(Data)
        schema_builder = JsonSchemaBuilder(ref_prefix='#/foo/bar')
        schema = schema_builder.build(serializer)
        definitions = schema_builder.get_definitions()

        assert schema == {
            '$ref': '#/foo/bar/tests.json_schema.test_convert.TestJsonSchemaBuilder.test_build__use_custom_ref_prefix.<locals>.Data',
        }

        assert definitions == {
            'tests.json_schema.test_convert.TestJsonSchemaBuilder.test_build__use_custom_ref_prefix.<locals>.Data': {
                'properties': {'a': {'type': 'integer'}},
                'required': ['a'],
                'type': 'object',
            }
        }

    def test_build__add_dialect_uri(self):
        serializer = Serializer(int)
        schema_builder = JsonSchemaBuilder(add_dialect_uri=True)
        schema = schema_builder.build(serializer)

        assert schema == {
            'type': 'integer',
            '$schema': 'https://json-schema.org/draft/2020-12/schema',
        }
        assert schema_builder.get_definitions() == {}
