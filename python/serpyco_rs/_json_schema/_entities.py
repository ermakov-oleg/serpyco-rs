from __future__ import annotations

from dataclasses import dataclass
from typing import Any


@dataclass
class Schema:
    type: str | None = None
    title: str | None = None
    description: str | None = None
    default: Any | None = None
    deprecated: bool | None = None
    enum: list[Any] | None = None

    allOf: list[Schema] | None = None
    anyOf: list[Schema] | None = None
    oneOf: list[Schema] | None = None

    def dump(self, definitions: dict[str, Any]) -> dict[str, Any]:
        data = {
            "type": self.type,
            "title": self.title,
            "description": self.description,
            "default": self.default,
            "deprecated": self.deprecated,
            "enum": self.enum,
            "allOf": [item.dump(definitions) for item in self.allOf] if self.allOf else None,
            "anyOf": [item.dump(definitions) for item in self.anyOf] if self.anyOf else None,
            "oneOf": [item.dump(definitions) for item in self.oneOf] if self.oneOf else None,
        }
        return {k: v for k, v in data.items() if v is not None}


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

    def dump(self, definitions: dict[str, Any]) -> dict[str, Any]:
        data = super().dump(definitions)
        data = {
            "minLength": self.minLength,
            "maxLength": self.maxLength,
            "pattern": self.pattern,
            "format": self.format,
            **data,
        }
        return {k: v for k, v in data.items() if v is not None}


@dataclass
class NumberType(Schema):
    type: str = "number"
    multipleOf: int | None = None
    minimum: float | None = None
    maximum: float | None = None

    def dump(self, definitions: dict[str, Any]) -> dict[str, Any]:
        data = super().dump(definitions)
        data = {
            "multipleOf": self.multipleOf,
            "minimum": self.minimum,
            "maximum": self.maximum,
            **data,
        }
        return {k: v for k, v in data.items() if v is not None}


@dataclass
class IntegerType(NumberType):
    type: str = "integer"


@dataclass
class ObjectType(Schema):
    name: str | None = None
    type: str = "object"
    properties: dict[str, Schema] | None = None
    additionalProperties: bool | Schema | None = None
    required: list[str] | None = None

    def dump(self, definitions: dict[str, Any]) -> dict[str, Any]:
        data = super().dump(definitions)
        data = {
            "properties": {k: v.dump(definitions) for k, v in self.properties.items()} if self.properties else None,
            "additionalProperties": (
                self.additionalProperties.dump(definitions)
                if isinstance(self.additionalProperties, Schema)
                else self.additionalProperties
            ),
            "required": self.required,
            **data,
        }
        data = {k: v for k, v in data.items() if v is not None}
        if not self.name:
            return data
        definitions[self.name] = data
        return {
            "$ref": f"#/definitions/{self.name}",
        }


@dataclass
class ArrayType(Schema):
    type: str = "array"
    items: Schema | None = None
    prefixItems: list[Schema] | None = None
    minItems: int | None = None
    maxItems: int | None = None
    uniqueItems: bool | None = None

    def dump(self, definitions: dict[str, Any]) -> dict[str, Any]:
        data = super().dump(definitions)
        data = {
            "items": self.items.dump(definitions) if self.items else None,
            "prefixItems": [i.dump(definitions) for i in self.prefixItems] if self.prefixItems else None,
            "minItems": self.minItems,
            "maxItems": self.maxItems,
            "uniqueItems": self.uniqueItems,
            **data,
        }
        return {k: v for k, v in data.items() if v is not None}


@dataclass
class RefType(Schema):
    ref: str | None = None

    def dump(self, definitions: dict[str, Any]) -> dict[str, Any]:
        data = super().dump(definitions)
        return {
            "$ref": self.ref,
            **data,
        }
