from functools import singledispatch
from typing import Any

from .. import _describe as describe
from ._entities import (
    Schema,
    ArrayType,
    ObjectType,
    Null,
    StringType,
    Boolean,
    NumberType,
    IntegerType,
)


@singledispatch
def to_json_schema(_: Any, doc: str | None = None) -> Schema:
    return Schema(description=doc)


@to_json_schema.register
def _(arg: describe.StringType, doc: str | None = None) -> Schema:
    return StringType(
        minLength=arg.min_length,
        maxLength=arg.max_length,
        description=doc,
    )


@to_json_schema.register
def _(arg: describe.IntegerType, doc: str | None = None) -> Schema:
    return IntegerType(
        minimum=arg.min,
        maximum=arg.max,
        description=doc,
    )


@to_json_schema.register
def _(_: describe.BytesType, doc: str | None = None) -> Schema:
    return StringType(
        format="binary",
        description=doc,
    )


@to_json_schema.register
def _(arg: describe.FloatType, doc: str | None = None) -> Schema:
    return NumberType(
        minimum=arg.min,
        maximum=arg.max,
        description=doc,
    )


@to_json_schema.register
def _(_: describe.DecimalType, doc: str | None = None) -> Schema:
    return Schema(
        oneOf=[
            StringType(),
            NumberType(),
        ],
        description=doc,
    )


@to_json_schema.register
def _(_: describe.BooleanType, doc: str | None = None) -> Schema:
    return Boolean()


@to_json_schema.register
def _(_: describe.UUIDType, doc: str | None = None) -> Schema:
    return StringType(
        format="uuid",
        description=doc,
    )


@to_json_schema.register
def _(_: describe.TimeType, doc: str | None = None) -> Schema:
    iso8601_pattern = (
        r"^[0-9][0-9]:[0-9][0-9](:[0-9][0-9](\.[0-9]+)?)?"  # HH:mm:ss.ssss
        r"?(([+-][0-9][0-9]:?[0-9][0-9])|Z)?$"  # timezone
    )
    return StringType(
        format="time",
        pattern=iso8601_pattern,
        description=doc,
    )


@to_json_schema.register
def _(_: describe.DateTimeType, doc: str | None = None) -> Schema:
    iso8601_pattern = (
        r"^[0-9]{4}-[0-9][0-9]-[0-9][0-9]T"  # YYYY-MM-DD
        r"[0-9][0-9]:[0-9][0-9]:[0-9][0-9](\.[0-9]+)"  # HH:mm:ss.ssss
        r"?(([+-][0-9][0-9]:[0-9][0-9])|Z)?$"  # timezone
    )
    return StringType(
        format="date-time",
        pattern=iso8601_pattern,
        description=doc,
    )


@to_json_schema.register
def _(_: describe.DateType, doc: str | None = None) -> Schema:
    iso8601_pattern = r"^[0-9]{4}-[0-9][0-9]-[0-9][0-9]$"  # YYYY-MM-DD
    return StringType(
        format="date",
        pattern=iso8601_pattern,
        description=doc,
    )


@to_json_schema.register
def _(arg: describe.EnumType, doc: str | None = None) -> Schema:
    return Schema(
        enum=[item.value for item in arg.cls],
        description=doc,
    )


@to_json_schema.register
def _(arg: describe.OptionalType, doc: str | None = None) -> Schema:
    return Schema(
        anyOf=[
            Null(),
            to_json_schema(arg.inner),
        ],
        description=doc,
    )


@to_json_schema.register
def _(arg: describe.EntityType, doc: str | None = None) -> Schema:
    return ObjectType(
        properties={
            prop.name: to_json_schema(prop.type, prop.doc)
            for prop in arg.fields
            if not prop.is_property
        },
        required=[
            prop.name
            for prop in arg.fields
            if not isinstance(prop.type, describe.OptionalType) and not prop.is_property
        ],
        description=arg.doc,
    )


@to_json_schema.register
def _(arg: describe.ArrayType, doc: str | None = None) -> Schema:
    return ArrayType(
        items=to_json_schema(arg.item_type),
        description=doc,
    )


@to_json_schema.register
def _(arg: describe.DictionaryType, doc: str | None = None) -> Schema:
    return ObjectType(
        additionalProperties=to_json_schema(arg.value_type),
        description=doc,
    )


@to_json_schema.register
def _(arg: describe.TupleType, doc: str | None = None) -> Schema:
    return ArrayType(
        prefixItems=[to_json_schema(item) for item in arg.item_types],
        minItems=len(arg.item_types),
        maxItems=len(arg.item_types),
        description=doc,
    )


@to_json_schema.register
def _(_: describe.AnyType, doc: str | None = None) -> Schema:
    return Schema(description=doc)
