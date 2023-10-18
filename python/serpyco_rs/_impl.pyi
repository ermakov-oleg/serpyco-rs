from typing import Any, Generic, TypeVar, Callable
from collections.abc import Sequence
from enum import Enum, IntEnum

_T = TypeVar('_T')
_I = TypeVar('_I')
_O = TypeVar('_O')

class ValidationError(Exception):
    message: str

class ErrorItem:
    message: str
    instance_path: str

    def __init__(self, message: str, instance_path: str): ...

class SchemaValidationError(ValidationError):
    errors: list[ErrorItem]

class Serializer(Generic[_T]):
    def __init__(self, py_class: BaseType, schema: str, pass_through_bytes: bool): ...
    def dump(self, value: _T) -> Any: ...
    def load(self, data: Any, validate: bool) -> _T: ...

class CustomEncoder(Generic[_I, _O]):
    serialize: Callable[[_I], _O] | None
    deserialize: Callable[[_O], _I] | None

    def __init__(self, serialize: Callable[[_I], _O] | None = None, deserialize: Callable[[_O], _I] | None = None): ...

class BaseType:
    custom_encoder: CustomEncoder[Any, Any] | None

    def __init__(self, custom_encoder: CustomEncoder[Any, Any] | None): ...

class IntegerType(BaseType):
    min: int | None
    max: int | None

    def __init__(self, min: int | None, max: int | None, custom_encoder: CustomEncoder[Any, Any] | None): ...

class FloatType(BaseType):
    min: float | None
    max: float | None

    def __init__(self, min: float | None, max: float | None, custom_encoder: CustomEncoder[Any, Any] | None): ...

class DecimalType(BaseType):
    min: float | None
    max: float | None

    def __init__(self, min: float | None, max: float | None, custom_encoder: CustomEncoder[Any, Any] | None): ...

class StringType(BaseType):
    min_length: int | None
    max_length: int | None

    def __init__(
        self, min_length: int | None, max_length: int | None, custom_encoder: CustomEncoder[Any, Any] | None
    ): ...

class BooleanType(BaseType):
    def __init__(self, custom_encoder: CustomEncoder[Any, Any] | None): ...

class UUIDType(BaseType):
    def __init__(self, custom_encoder: CustomEncoder[Any, Any] | None): ...

class TimeType(BaseType):
    def __init__(self, custom_encoder: CustomEncoder[Any, Any] | None): ...

class DateTimeType(BaseType):
    def __init__(self, custom_encoder: CustomEncoder[Any, Any] | None): ...

class DateType(BaseType):
    def __init__(self, custom_encoder: CustomEncoder[Any, Any] | None): ...

class DefaultValue(Generic[_T]):
    @staticmethod
    def none() -> DefaultValue[_T]: ...
    @staticmethod
    def some(value: _T) -> DefaultValue[_T]: ...
    def is_none(self) -> bool: ...

NOT_SET: DefaultValue[None]

class EntityField(BaseType):
    name: str
    dict_key: str
    field_type: BaseType
    required: bool = True
    is_discriminator_field: bool = False
    default: DefaultValue[Any]
    default_factory: DefaultValue[Callable[[], Any]]
    doc: str | None

    def __init__(
        self,
        name: str,
        dict_key: str,
        field_type: BaseType,
        required: bool = True,
        is_discriminator_field: bool = False,
        default: DefaultValue[Any] = ...,
        default_factory: DefaultValue[Callable[[], Any]] = ...,
        doc: str | None = None,
    ): ...

class EntityType(BaseType):
    cls: type[Any]
    name: str
    fields: Sequence[EntityField]
    omit_none: bool
    generics: Sequence[tuple[TypeVar, Any]]
    doc: str | None

    def __init__(
        self,
        cls: type[Any],
        name: str,
        fields: Sequence[EntityField],
        omit_none: bool = False,
        generics: Sequence[tuple[TypeVar, Any]] | None = None,
        doc: str | None = None,
        custom_encoder: CustomEncoder[Any, Any] | None = None,
    ): ...

class TypedDictType(BaseType):
    name: str
    fields: Sequence[EntityField]
    omit_none: bool
    generics: Sequence[tuple[TypeVar, Any]]
    doc: str | None

    def __init__(
        self,
        name: str,
        fields: Sequence[EntityField],
        omit_none: bool = False,
        generics: Sequence[tuple[TypeVar, Any]] | None = None,
        doc: str | None = None,
        custom_encoder: CustomEncoder[Any, Any] | None = None,
    ): ...

class ArrayType(BaseType):
    item_type: BaseType

    def __init__(self, item_type: BaseType, custom_encoder: CustomEncoder[Any, Any] | None = None): ...

class EnumType(BaseType):
    cls: type[Enum | IntEnum]
    items: list[Any]

    def __init__(
        self, cls: type[Enum | IntEnum], items: list[Any], custom_encoder: CustomEncoder[Any, Any] | None = None
    ): ...

class OptionalType(BaseType):
    inner: BaseType

    def __init__(self, inner: BaseType, custom_encoder: CustomEncoder[Any, Any] | None = None): ...

class DictionaryType(BaseType):
    key_type: BaseType
    value_type: BaseType
    omit_none: bool

    def __init__(
        self,
        key_type: BaseType,
        value_type: BaseType,
        omit_none: bool = False,
        custom_encoder: CustomEncoder[Any, Any] | None = None,
    ): ...


class TupleType(BaseType):
    item_types: list[BaseType]

    def __init__(self, item_types: list[BaseType], custom_encoder: CustomEncoder[Any, Any] | None = None): ...


class BytesType(BaseType):

    def __init__(self, custom_encoder: CustomEncoder[Any, Any] | None = None): ...

