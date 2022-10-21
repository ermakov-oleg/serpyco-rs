from __future__ import annotations

from dataclasses import dataclass, asdict
from typing import Any


@dataclass
class Schema:
    title: str | None = None
    description: str | None = None
    default: Any | None = None
    deprecated: bool | None = None
    enum: list[Any] | None = None

    allOf: list[Schema] | None = None
    anyOf: list[Schema] | None = None
    oneOf: list[Schema] | None = None

    def dump(self) -> dict[str, Any]:
        def factory(items: list[tuple[str, Any]]):
            return dict(((k, v) for k, v in items if v is not None))

        schema = asdict(self, dict_factory=factory)
        schema["$schema"] = "https://json-schema.org/draft/2020-12/schema"
        return schema


@dataclass
class Boolean(Schema):
    type: str = "boolean"


@dataclass
class Null(Schema):
    type: str = "null"


@dataclass
class StringType(Schema):
    type: str = "string"
    minLength: int | None = None
    maxLength: int | None = None
    pattern: str | None = None
    format: str | None = None
    # todo: enum https://json-schema.org/understanding-json-schema/reference/string.html#built-in-formats


@dataclass
class NumberType(Schema):
    type: str = "number"
    multipleOf: int | None = None
    minimum: int | None = None
    exclusiveMinimum: int | None = None
    maximum: int | None = None
    exclusiveMaximum: int | None = None


@dataclass
class IntegerType(NumberType):
    type: str = "integer"


@dataclass
class ObjectType(Schema):
    type: str = "object"
    properties: dict[str, Schema] | None = None
    additionalProperties: bool | Schema | None = None
    required: list[str] | None = None


@dataclass
class ArrayType(Schema):
    type: str = "array"
    items: Schema | None = None
    prefixItems: list[Schema] | None = None
    minItems: int | None = None
    maxItems: int | None = None
    uniqueItems: bool | None = None
