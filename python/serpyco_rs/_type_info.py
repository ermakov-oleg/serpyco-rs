import dataclasses
from collections.abc import Callable, Mapping, Sequence
from enum import Enum, IntEnum
from typing import Any, Generic, TypeVar


_I = TypeVar('_I')
_O = TypeVar('_O')


@dataclasses.dataclass(slots=True, kw_only=True)
class CustomEncoder(Generic[_I, _O]):
    serialize: Callable[[_I], _O] | None = None
    deserialize: Callable[[_O], _I] | None = None


@dataclasses.dataclass(slots=True, kw_only=True)
class BaseType:
    custom_encoder: CustomEncoder[Any, Any] | None = None
    json_schema_extensions: Mapping[str, Any] | None = None


@dataclasses.dataclass(slots=True, kw_only=True)
class ContainerBaseType(BaseType):
    ref_name: str = dataclasses.field(compare=False)
    _usage_count: int = dataclasses.field(default=0, compare=False, repr=False)

    def set_usages(self, value: int) -> None:
        self._usage_count = value

    def should_use_ref(self) -> bool:
        return self._usage_count > 1


class _NotSet(Enum):
    token = 0


NOT_SET = _NotSet.token


@dataclasses.dataclass(slots=True, kw_only=True)
class NoneType(BaseType):
    pass


@dataclasses.dataclass(slots=True, kw_only=True)
class NeverType(BaseType):
    pass


@dataclasses.dataclass(slots=True, kw_only=True)
class IntegerType(BaseType):
    min: int | None = None
    max: int | None = None
    inclusive_min: bool = True
    inclusive_max: bool = True


@dataclasses.dataclass(slots=True, kw_only=True)
class FloatType(BaseType):
    min: float | None = None
    max: float | None = None
    inclusive_min: bool = True
    inclusive_max: bool = True


@dataclasses.dataclass(slots=True, kw_only=True)
class DecimalType(BaseType):
    min: float | None = None
    max: float | None = None
    inclusive_min: bool = True
    inclusive_max: bool = True


@dataclasses.dataclass(slots=True, kw_only=True)
class StringType(BaseType):
    min_length: int | None = None
    max_length: int | None = None


@dataclasses.dataclass(slots=True, kw_only=True)
class BooleanType(BaseType):
    pass


@dataclasses.dataclass(slots=True, kw_only=True)
class UUIDType(BaseType):
    pass


@dataclasses.dataclass(slots=True, kw_only=True)
class TimeType(BaseType):
    pass


@dataclasses.dataclass(slots=True, kw_only=True)
class DateTimeType(BaseType):
    pass


@dataclasses.dataclass(slots=True, kw_only=True)
class DateType(BaseType):
    pass


@dataclasses.dataclass(slots=True, kw_only=True)
class BytesType(BaseType):
    pass


@dataclasses.dataclass(slots=True, kw_only=True)
class AnyType(BaseType):
    pass


@dataclasses.dataclass(slots=True, kw_only=True)
class EntityField:
    name: str
    dict_key: str
    field_type: BaseType
    required: bool = True
    is_discriminator_field: bool = False
    default: Any | _NotSet = NOT_SET
    default_factory: Callable[[], Any] | _NotSet = NOT_SET
    doc: str | None = None
    is_flattened: bool = False
    is_dict_flatten: bool = False


@dataclasses.dataclass(slots=True, kw_only=True)
class EntityType(BaseType):
    cls: type[Any]
    name: str
    fields: Sequence[EntityField]
    omit_none: bool = False
    is_frozen: bool = dataclasses.field(default=False, compare=False)
    used_keys: set[str] | None = dataclasses.field(default=None, compare=False)
    doc: str | None = None


@dataclasses.dataclass(slots=True, kw_only=True)
class TypedDictType(BaseType):
    name: str
    fields: Sequence[EntityField]
    omit_none: bool = False
    doc: str | None = None
    used_keys: set[str] | None = dataclasses.field(default=None, compare=False)


@dataclasses.dataclass(slots=True, kw_only=True)
class ArrayType(ContainerBaseType):
    item_type: BaseType
    min_length: int | None = None
    max_length: int | None = None


@dataclasses.dataclass(slots=True, kw_only=True)
class EnumType(BaseType):
    cls: type[Enum | IntEnum]
    items: list[Any]


@dataclasses.dataclass(slots=True, kw_only=True)
class OptionalType(BaseType):
    inner: BaseType


@dataclasses.dataclass(slots=True, kw_only=True)
class DictionaryType(BaseType):
    key_type: BaseType
    value_type: BaseType
    omit_none: bool = False


@dataclasses.dataclass(slots=True, kw_only=True)
class TupleType(ContainerBaseType):
    item_types: list[BaseType]


@dataclasses.dataclass(slots=True, kw_only=True)
class UnionType(ContainerBaseType):
    item_types: list[BaseType]

    @property
    def repr(self) -> str:
        return self.ref_name


@dataclasses.dataclass(slots=True, kw_only=True)
class DiscriminatedUnionType(ContainerBaseType):
    item_types: dict[str, BaseType]
    dump_discriminator: str
    load_discriminator: str


@dataclasses.dataclass(slots=True, kw_only=True)
class LiteralType(BaseType):
    args: list[str | int | Enum]


@dataclasses.dataclass(slots=True, kw_only=True)
class RecursionHolder(BaseType):
    name: str
    state_key: str
    meta: Any


@dataclasses.dataclass(slots=True, kw_only=True)
class CustomType(BaseType):
    json_schema: dict[str, Any]
