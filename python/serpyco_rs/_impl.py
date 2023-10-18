from typing import Generic, TypeVar

from ._serpyco_rs import (
    AnyType,
    ArrayType,
    BaseType,
    BooleanType,
    BytesType,
    CustomEncoder as _CustomEncoder,
    DateTimeType,
    DateType,
    DecimalType,
    DefaultValue as _DefaultValue,
    DictionaryType,
    EntityField,
    EntityType,
    EnumType,
    ErrorItem,
    FloatType,
    IntegerType,
    LiteralType,
    OptionalType,
    RecursionHolder,
    SchemaValidationError,
    Serializer,
    StringType,
    TimeType,
    TupleType,
    TypedDictType,
    UnionType,
    UUIDType,
    ValidationError,
)


_T = TypeVar('_T')
_I = TypeVar('_I')
_O = TypeVar('_O')


class CustomEncoder(_CustomEncoder, Generic[_I, _O]):
    """pyo3 doesn't support specifying concrete types for generic methods."""


class DefaultValue(_DefaultValue, Generic[_T]):
    """pyo3 doesn't support specifying concrete types for generic methods."""


NOT_SET = DefaultValue.none()
