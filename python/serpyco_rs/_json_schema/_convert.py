import sys
from functools import singledispatch
from typing import Any, Optional, Union, cast

if sys.version_info >= (3, 10):
    from typing import TypeGuard
else:
    from typing_extensions import TypeGuard

from .. import _describe as describe
from ._entities import (
    ArrayType,
    Boolean,
    Discriminator,
    IntegerType,
    Null,
    NumberType,
    ObjectType,
    RefType,
    Schema,
    StringType,
    UnionType,
)


def get_json_schema(t: describe.BaseType) -> dict[str, Any]:
    schema = to_json_schema(t)
    definitions: dict[str, Any] = {}
    schema_def = schema.dump(definitions)
    components = {'components': {'schemas': definitions}} if definitions else {}
    return {
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        **schema_def,
        **components,
    }


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
        format='binary',
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
            StringType(
                format='decimal',
            ),
            NumberType(
                format='decimal',
            ),
        ],
        description=doc,
    )


@to_json_schema.register
def _(_: describe.BooleanType, doc: Optional[str] = None) -> Schema:
    return Boolean()


@to_json_schema.register
def _(_: describe.UUIDType, doc: Optional[str] = None) -> Schema:
    return StringType(
        format='uuid',
        description=doc,
    )


@to_json_schema.register
def _(_: describe.TimeType, doc: Optional[str] = None) -> Schema:
    return StringType(
        format='time',
        description=doc,
    )


@to_json_schema.register
def _(_: describe.DateTimeType, doc: Optional[str] = None) -> Schema:
    return StringType(
        format='date-time',
        description=doc,
    )


@to_json_schema.register
def _(_: describe.DateType, doc: Optional[str] = None) -> Schema:
    return StringType(
        format='date',
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
        properties={prop.dict_key: to_json_schema(prop.field_type, prop.doc) for prop in arg.fields},
        required=[prop.dict_key for prop in arg.fields if prop.required] or None,
        name=arg.name,
        description=arg.doc,
    )


@to_json_schema.register
def _(arg: describe.TypedDictType, doc: Optional[str] = None) -> Schema:
    return ObjectType(
        properties={prop.dict_key: to_json_schema(prop.field_type, prop.doc) for prop in arg.fields},
        required=[prop.dict_key for prop in arg.fields if prop.required] or None,
        name=arg.name,
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


@to_json_schema.register
def _(holder: describe.RecursionHolder, doc: Optional[str] = None) -> Schema:
    return RefType(description=doc, ref=f'#/components/schemas/{holder.name}')


@to_json_schema.register
def _(arg: describe.LiteralType, doc: Optional[str] = None) -> Schema:
    return Schema(
        enum=list(arg.args),
        description=doc,
    )


@to_json_schema.register
def _(arg: describe.UnionType, doc: Optional[str] = None) -> Schema:
    objects = {
        name: schema
        for name, t in arg.item_types.items()
        if (schema := to_json_schema(t)) and _check_unions_schema_types(schema)
    }

    return UnionType(
        oneOf=list(objects.values()),
        discriminator=Discriminator(
            property_name=arg.load_discriminator,
            mapping={name: cast(str, val.ref) for name, val in objects.items()},
        ),
        description=doc,
    )


def _check_unions_schema_types(schema: Schema) -> TypeGuard[Union[ObjectType, RefType]]:
    if isinstance(schema, (ObjectType, RefType)):
        return True
    raise RuntimeError(f'Unions schema items must be ObjectType or RefType. Current: {schema}')
