import sys
from dataclasses import dataclass
from datetime import datetime, time
from decimal import Decimal
from enum import Enum
from typing import Annotated, Any, Optional
from uuid import UUID

import pytest
from serpyco_rs._describe import describe_type
from serpyco_rs._json_schema import get_json_schema
from serpyco_rs.metadata import CamelCase, Max, MaxLength, Min, MinLength


def test_to_json_schema():
    class EnumCls(Enum):
        a = "a"

    @dataclass
    class InnerData:
        """Some important entity"""

        foo: str

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
        some_filed: Annotated[str, CamelCase]

    schema = get_json_schema(describe_type(Data))

    assert schema == {
        "$ref": "#/definitions/tests._json_schema.test_convert.Data[no_format]",
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "definitions": {
            "tests._json_schema.test_convert.Data[no_format]": {
                "description": "Docs",
                "properties": {
                    "a": {
                        "description": "A " "field " "with " "bounds",
                        "maximum": 10,
                        "minimum": 0,
                        "type": "integer",
                    },
                    "b": {"type": "number"},
                    "c": {"oneOf": [{"type": "string"}, {"type": "number"}]},
                    "d": {"type": "boolean"},
                    "e": {"maxLength": 5, "minLength": 1, "type": "string"},
                    "f": {"format": "uuid", "type": "string"},
                    "g": {
                        "format": "regex",
                        "pattern": "^[0-9][0-9]:[0-9][0-9](:[0-9][0-9](\\.[0-9]+)?)??(([+-][0-9][0-9]:?[0-9][0-9])|Z)?$",
                        "type": "string",
                    },
                    "h": {
                        "format": "regex",
                        "pattern": "^[0-9]{4}-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9](\\.[0-9]+)?(([+-][0-9][0-9]:[0-9][0-9])|Z)?$",
                        "type": "string",
                    },
                    "i": {"enum": ["a"]},
                    "j": {"$ref": "#/definitions/tests._json_schema.test_convert.InnerData[no_format]"},
                    "k": {"items": {"type": "integer"}, "type": "array"},
                    "l": {
                        "maxItems": 3,
                        "minItems": 3,
                        "prefixItems": [
                            {"type": "integer"},
                            {"type": "string"},
                            {"$ref": "#/definitions/tests._json_schema.test_convert.InnerData[no_format]"},
                        ],
                        "type": "array",
                    },
                    "m": {"additionalProperties": {"type": "integer"}, "type": "object"},
                    "n": {},
                    "someFiled": {"type": "string"},
                },
                "required": ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "someFiled"],
                "type": "object",
            },
            "tests._json_schema.test_convert.InnerData[no_format]": {
                "description": "Some " "important " "entity",
                "properties": {"foo": {"type": "string"}},
                "required": ["foo"],
                "type": "object",
            },
        },
    }


@pytest.mark.skipif(sys.version_info < (3, 10), reason="New style unions available after 3.10")
def test_to_json_schema__new_union_syntax():
    @dataclass
    class Data:
        """Docs"""

        a: int | None

    schema = get_json_schema(describe_type(Data))

    assert schema == {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/definitions/tests._json_schema.test_convert.Data[no_format]",
        "definitions": {
            "tests._json_schema.test_convert.Data[no_format]": {
                "description": "Docs",
                "properties": {"a": {"anyOf": [{"type": "null"}, {"type": "integer"}]}},
                "required": ["a"],
                "type": "object",
            }
        },
    }


@dataclass
class TreeNode:
    """Node"""

    data: str
    left: Optional["TreeNode"] = None
    right: Optional["TreeNode"] = None


def test_to_json_schema__recursive_type():

    schema = get_json_schema(describe_type(TreeNode))

    assert schema == {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/definitions/tests._json_schema.test_convert.TreeNode[no_format]",
        "definitions": {
            "tests._json_schema.test_convert.TreeNode[no_format]": {
                "description": "Node",
                "properties": {
                    "data": {"type": "string"},
                    "left": {
                        "anyOf": [
                            {"type": "null"},
                            {"$ref": "#/definitions/tests._json_schema.test_convert.TreeNode[no_format]"},
                        ]
                    },
                    "right": {
                        "anyOf": [
                            {"type": "null"},
                            {"$ref": "#/definitions/tests._json_schema.test_convert.TreeNode[no_format]"},
                        ]
                    },
                },
                "required": ["data"],
                "type": "object",
            }
        },
    }
