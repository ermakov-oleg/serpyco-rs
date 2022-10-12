import sys
from dataclasses import dataclass
from datetime import time, datetime
from decimal import Decimal
from enum import Enum
from typing import Any, Annotated
from uuid import UUID

from serpyco_rs._json_schema import to_json_schema
from serpyco_rs._describe import describe_type
from serpyco_rs.metadata import Min, Max, MinLength, MaxLength


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

    schema = to_json_schema(describe_type(Data)).dump()

    assert schema == {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "description": "Docs",
        "properties": {
            "a": {
                "description": "A field with bounds",
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
            "j": {
                "description": "Some important entity",
                "properties": {"foo": {"type": "string"}},
                "required": ["foo"],
                "type": "object",
            },
            "k": {"items": {"type": "integer"}, "type": "array"},
            "l": {
                "maxItems": 3,
                "minItems": 3,
                "prefixItems": [
                    {"type": "integer"},
                    {"type": "string"},
                    {
                        "description": "Some important entity",
                        "properties": {"foo": {"type": "string"}},
                        "required": ["foo"],
                        "type": "object",
                    },
                ],
                "type": "array",
            },
            "m": {"additionalProperties": {"type": "integer"}, "type": "object"},
            "n": {},
        },
        "required": [
            "a",
            "b",
            "c",
            "d",
            "e",
            "f",
            "g",
            "h",
            "i",
            "j",
            "k",
            "l",
            "m",
            "n",
        ],
        "type": "object",
    }


if sys.version_info >= (3, 10):

    def test_to_json_schema__new_union_syntax():
        @dataclass
        class Data:
            """Docs"""

            a: int | None

        schema = to_json_schema(describe_type(Data)).dump()

        assert schema == {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "description": "Docs",
            "properties": {
                "a": {"anyOf": [{"type": "null"}, {"type": "integer"}]},
            },
            "required": ["a"],
            "type": "object",
        }
