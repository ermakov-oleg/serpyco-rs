from functools import singledispatch
from typing import Any, Optional

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
def to_json_schema(_: Any, doc: Optional[str] = None) -> Schema:
    return Schema(description=doc)


@to_json_schema.register
def _(arg: describe.StringType, doc: Optional[str] = None) -> Schema:
    return StringType(
        minLength=arg.min_length,
        maxLength=arg.max_length,
        description=doc,
    )


@to_json_schema.register
def _(arg: describe.IntegerType, doc: Optional[str] = None) -> Schema:
    return IntegerType(
        minimum=arg.min,
        maximum=arg.max,
        description=doc,
    )


@to_json_schema.register
def _(_: describe.BytesType, doc: Optional[str] = None) -> Schema:
    return StringType(
        format="binary",
        description=doc,
    )


@to_json_schema.register
def _(arg: describe.FloatType, doc: Optional[str] = None) -> Schema:
    return NumberType(
        minimum=arg.min,
        maximum=arg.max,
        description=doc,
    )


@to_json_schema.register
def _(_: describe.DecimalType, doc: Optional[str] = None) -> Schema:
    return Schema(
        oneOf=[
            StringType(),
            NumberType(),
        ],
        description=doc,
    )


@to_json_schema.register
def _(_: describe.BooleanType, doc: Optional[str] = None) -> Schema:
    return Boolean()


@to_json_schema.register
def _(_: describe.UUIDType, doc: Optional[str] = None) -> Schema:
    return StringType(
        format="uuid",
        description=doc,
    )


@to_json_schema.register
def _(_: describe.TimeType, doc: Optional[str] = None) -> Schema:
    iso8601_pattern = (
        r"^[0-9][0-9]:[0-9][0-9](:[0-9][0-9](\.[0-9]+)?)?"  # HH:mm:ss.ssss
        r"?(([+-][0-9][0-9]:?[0-9][0-9])|Z)?$"  # timezone
    )
    return StringType(
        format="regex",
        pattern=iso8601_pattern,
        description=doc,
    )


@to_json_schema.register
def _(_: describe.DateTimeType, doc: Optional[str] = None) -> Schema:
    iso8601_pattern = (
        r"^[0-9]{4}-[0-9][0-9]-[0-9][0-9]T"  # YYYY-MM-DD
        r"[0-9][0-9]:[0-9][0-9]:[0-9][0-9](\.[0-9]+)"  # HH:mm:ss.ssss
        r"?(([+-][0-9][0-9]:[0-9][0-9])|Z)?$"  # timezone
    )
    return StringType(
        format="regex",
        pattern=iso8601_pattern,
        description=doc,
    )


@to_json_schema.register
def _(_: describe.DateType, doc: Optional[str] = None) -> Schema:
    iso8601_pattern = r"^[0-9]{4}-[0-9][0-9]-[0-9][0-9]$"  # YYYY-MM-DD
    return StringType(
        format="regex",
        pattern=iso8601_pattern,
        description=doc,
    )


@to_json_schema.register
def _(arg: describe.EnumType, doc: Optional[str] = None) -> Schema:
    return Schema(
        enum=[item.value for item in arg.cls],
        description=doc,
    )


@to_json_schema.register
def _(arg: describe.OptionalType, doc: Optional[str] = None) -> Schema:
    return Schema(
        anyOf=[
            Null(),
            to_json_schema(arg.inner),
        ],
        description=doc,
    )


@to_json_schema.register
def _(arg: describe.EntityType, doc: Optional[str] = None) -> Schema:
    return ObjectType(
        properties={
            prop.name: to_json_schema(prop.type, prop.doc)
            for prop in arg.fields
            if not prop.is_property
        },
        required=[
            prop.name
            for prop in arg.fields
            if not (
                prop.is_property
                or prop.default != describe.NOT_SET
                or prop.default_factory != describe.NOT_SET
            )
        ]
        or None,
        description=arg.doc,
    )


@to_json_schema.register
def _(arg: describe.ArrayType, doc: Optional[str] = None) -> Schema:
    return ArrayType(
        items=to_json_schema(arg.item_type),
        description=doc,
    )


@to_json_schema.register
def _(arg: describe.DictionaryType, doc: Optional[str] = None) -> Schema:
    return ObjectType(
        additionalProperties=to_json_schema(arg.value_type),
        description=doc,
    )


@to_json_schema.register
def _(arg: describe.TupleType, doc: Optional[str] = None) -> Schema:
    return ArrayType(
        prefixItems=[to_json_schema(item) for item in arg.item_types],
        minItems=len(arg.item_types),
        maxItems=len(arg.item_types),
        description=doc,
    )


@to_json_schema.register
def _(_: describe.AnyType, doc: Optional[str] = None) -> Schema:
    return Schema(description=doc)
