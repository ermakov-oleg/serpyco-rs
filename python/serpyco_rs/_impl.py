from typing import Generic, TypeVar

from ._serpyco_rs import (
    BaseType,
    BooleanType,
    CustomEncoder as _CustomEncoder,
    DateTimeType,
    DateType,
    DecimalType,
    DefaultValue as _DefaultValue,
    EntityField,
    EntityType,
    ErrorItem,
    FloatType,
    IntegerType,
    SchemaValidationError,
    Serializer,
    StringType,
    TimeType,
    UUIDType,
    ValidationError,
    TypedDictType,
)


_T = TypeVar('_T')
_I = TypeVar('_I')
_O = TypeVar('_O')


class CustomEncoder(_CustomEncoder, Generic[_I, _O]):
    """pyo3 doesn't support specifying concrete types for generic methods."""


class DefaultValue(_DefaultValue, Generic[_T]):
    """pyo3 doesn't support specifying concrete types for generic methods."""


NOT_SET = DefaultValue.none()
