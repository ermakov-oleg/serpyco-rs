import sys
from dataclasses import dataclass
from datetime import datetime, time
from decimal import Decimal
from enum import Enum
from typing import Annotated, Any, Literal, Optional, TypedDict, Union
from unittest import mock
from uuid import UUID

import pytest
from serpyco_rs import Serializer
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
        '$ref': '#/components/schemas/tests.json_schema.test_convert.Data[no_format,keep_nones]',
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        'components': {
            'schemas': {
                'tests.json_schema.test_convert.Data[no_format,keep_nones]': {
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
                        'i': {'enum': ['a']},
                        'j': {
                            '$ref': '#/components/schemas/tests.json_schema.test_convert.InnerData[no_format,keep_nones]'
                        },
                        'k': {'items': {'type': 'integer'}, 'type': 'array'},
                        'l': {
                            'maxItems': 3,
                            'minItems': 3,
                            'prefixItems': [
                                {'type': 'integer'},
                                {'type': 'string'},
                                {
                                    '$ref': '#/components/schemas/tests.json_schema.test_convert.InnerData[no_format,keep_nones]'
                                },
                            ],
                            'type': 'array',
                        },
                        'm': {'additionalProperties': {'type': 'integer'}, 'type': 'object'},
                        'n': {},
                        'p': {
                            '$ref': '#/components/schemas/tests.json_schema.test_convert.OtherInnerData[no_format,omit_nones]'
                        },
                        'o': {
                            '$ref': '#/components/schemas/tests.json_schema.test_convert.InnerData[camel_case,keep_nones]'
                        },
                        'q': {
                            '$ref': '#/components/schemas/tests.json_schema.test_convert.AnotherInnerData[no_format,keep_nones]'
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
                'tests.json_schema.test_convert.InnerData[no_format,keep_nones]': {
                    'description': 'Some important entity',
                    'properties': {'foo_filed': {'type': 'string'}},
                    'required': ['foo_filed'],
                    'type': 'object',
                },
                'tests.json_schema.test_convert.InnerData[no_format,omit_nones]': {
                    'description': 'Some important entity',
                    'properties': {'foo_filed': {'type': 'string'}},
                    'required': ['foo_filed'],
                    'type': 'object',
                },
                'tests.json_schema.test_convert.InnerData[camel_case,keep_nones]': {
                    'description': 'Some important entity',
                    'properties': {'fooFiled': {'type': 'string'}},
                    'required': ['fooFiled'],
                    'type': 'object',
                },
                'tests.json_schema.test_convert.OtherInnerData[no_format,omit_nones]': {
                    'description': 'OtherInnerData',
                    'properties': {
                        'optional_filed': {
                            'anyOf': [
                                {'type': 'null'},
                                {
                                    '$ref': '#/components/schemas/tests.json_schema.test_convert.InnerData[no_format,omit_nones]'
                                },
                            ]
                        }
                    },
                    'type': 'object',
                },
                'tests.json_schema.test_convert.AnotherInnerData[no_format,keep_nones]': {
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
        '$ref': '#/components/schemas/tests.json_schema.test_convert.Data[no_format,keep_nones]',
        'components': {
            'schemas': {
                'tests.json_schema.test_convert.Data[no_format,keep_nones]': {
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
        '$ref': '#/components/schemas/tests.json_schema.test_convert.TreeNode[no_format,keep_nones]',
        'components': {
            'schemas': {
                'tests.json_schema.test_convert.TreeNode[no_format,keep_nones]': {
                    'description': 'Node',
                    'properties': {
                        'data': {'type': 'string'},
                        'left': {
                            'anyOf': [
                                {'type': 'null'},
                                {
                                    '$ref': '#/components/schemas/tests.json_schema.test_convert.TreeNode[no_format,keep_nones]'
                                },
                            ]
                        },
                        'right': {
                            'anyOf': [
                                {'type': 'null'},
                                {
                                    '$ref': '#/components/schemas/tests.json_schema.test_convert.TreeNode[no_format,keep_nones]'
                                },
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
    serializer = Serializer(Literal['foo'])
    assert serializer.get_json_schema() == {
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        'enum': ['foo'],
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

    @dataclass
    class Base:
        field: Annotated[Union[Foo, Bar], Discriminator('type')]

    serializer = Serializer(Base)
    assert serializer.get_json_schema() == {
        '$ref': '#/components/schemas/tests.json_schema.test_convert.Base[no_format,keep_nones]',
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        'components': {
            'schemas': {
                'tests.json_schema.test_convert.Bar[no_format,keep_nones]': {
                    'properties': {'type': {'enum': ['bar']}, 'val': {'type': 'string'}},
                    'required': ['val', 'type'],
                    'type': 'object',
                },
                'tests.json_schema.test_convert.Base[no_format,keep_nones]': {
                    'properties': {
                        'field': {
                            'discriminator': {
                                'mapping': {
                                    'bar': '#/components/schemas/tests.json_schema.test_convert.Bar[no_format,keep_nones]',
                                    'foo': '#/components/schemas/tests.json_schema.test_convert.Foo[no_format,keep_nones]',
                                },
                                'propertyName': 'type',
                            },
                            'oneOf': [
                                {
                                    '$ref': '#/components/schemas/tests.json_schema.test_convert.Foo[no_format,keep_nones]'
                                },
                                {
                                    '$ref': '#/components/schemas/tests.json_schema.test_convert.Bar[no_format,keep_nones]'
                                },
                            ],
                        }
                    },
                    'required': ['field'],
                    'type': 'object',
                },
                'tests.json_schema.test_convert.Foo[no_format,keep_nones]': {
                    'properties': {'type': {'enum': ['foo']}, 'val': {'type': 'integer'}},
                    'required': ['val', 'type'],
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
        '$ref': '#/components/schemas/tests.json_schema.test_convert.Data[no_format,keep_nones,force_none]',
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        'components': {
            'schemas': {
                'tests.json_schema.test_convert.Data[no_format,keep_nones,force_none]': {
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
        '$ref': '#/components/schemas/tests.json_schema.test_convert.Data[no_format,keep_nones]',
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        'components': {
            'schemas': {
                'tests.json_schema.test_convert.Data[no_format,keep_nones]': {
                    'properties': {'a': {'type': 'string', 'format': 'binary'}},
                    'type': 'object',
                    'required': ['a'],
                }
            }
        },
    }
