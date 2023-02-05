import dataclasses
import sys
from collections.abc import Callable, Iterable, Mapping, Sequence
from datetime import date, datetime, time
from decimal import Decimal
from enum import Enum, IntEnum
from typing import Annotated, Any, Optional, TypeVar, Union, cast, get_origin, get_type_hints, overload
from uuid import UUID

from attributes_doc import get_attributes_doc
from typing_extensions import assert_never

from ._utils import to_camelcase
from .metadata import (
    Alias,
    FiledFormat,
    Format,
    KeepNone,
    Max,
    MaxLength,
    Min,
    MinLength,
    NoFormat,
    NoneFormat,
    OmitNone,
    Places,
)

if sys.version_info >= (3, 10):  # pragma: no cover
    from types import UnionType
else:  # pragma: no cover
    UnionType = None

try:
    import attr
except ImportError:  # pragma: no cover
    attr = None  # type: ignore


_NoneType = type(None)

_T = TypeVar("_T")


class NotSet:
    def __repr__(self) -> str:
        return "NOT_SET"


NOT_SET = NotSet()


@dataclasses.dataclass
class Type:
    pass


@dataclasses.dataclass
class IntegerType(Type):
    min: Optional[int] = None
    max: Optional[int] = None


@dataclasses.dataclass
class StringType(Type):
    min_length: Optional[int] = None
    max_length: Optional[int] = None


@dataclasses.dataclass
class BytesType(Type):
    pass


@dataclasses.dataclass
class FloatType(Type):
    min: Optional[float] = None
    max: Optional[float] = None


@dataclasses.dataclass
class DecimalType(Type):
    places: Optional[int] = None
    min: Optional[Decimal] = None
    max: Optional[Decimal] = None


@dataclasses.dataclass
class BooleanType(Type):
    pass


@dataclasses.dataclass
class UUIDType(Type):
    pass


@dataclasses.dataclass
class TimeType(Type):
    pass


@dataclasses.dataclass
class DateTimeType(Type):
    pass


@dataclasses.dataclass
class DateType(Type):
    pass


@dataclasses.dataclass
class EnumType(Type):
    cls: type[Union[Enum, IntEnum]]


@dataclasses.dataclass
class EntityField:
    name: str
    dict_key: str
    type: Type
    doc: Optional[str] = None
    default: Any = NOT_SET
    default_factory: Union[Callable[[], Any], NotSet] = NOT_SET
    is_property: bool = False

    @property
    def required(self) -> bool:
        return not (self.is_property or self.default != NOT_SET or self.default_factory != NOT_SET)


@dataclasses.dataclass
class EntityType(Type):
    cls: type[Any]
    name: str
    fields: Sequence[EntityField]
    omit_none: bool = False
    generics: Mapping[TypeVar, Any] = dataclasses.field(default_factory=dict)
    doc: Optional[str] = None


@dataclasses.dataclass
class OptionalType(Type):
    inner: Type


@dataclasses.dataclass
class ArrayType(Type):
    item_type: Type
    is_sequence: bool


@dataclasses.dataclass
class DictionaryType(Type):
    key_type: Type
    value_type: Type
    is_mapping: bool
    omit_none: bool = False


@dataclasses.dataclass
class TupleType(Type):
    item_types: Sequence[Type]


@dataclasses.dataclass
class AnyType(Type):
    pass


@dataclasses.dataclass
class RecursionHolder(Type):
    cls: Any
    name: str
    field_format: FiledFormat
    state: dict[tuple[type, FiledFormat, NoneFormat], Optional[Type]]
    none_format: NoneFormat = KeepNone

    def get_type(self) -> Type:
        if type_ := self.state[(self.cls, self.field_format, self.none_format)]:
            return type_
        raise RuntimeError("Recursive type not resolved")


def describe_type(t: Any, state: Optional[dict[tuple[type, FiledFormat, NoneFormat], Optional[Type]]] = None) -> Type:
    state = state or {}
    parameters: tuple[Any, ...] = ()
    args: tuple[Any, ...] = ()
    metadata = _get_annotated_metadata(t)
    if get_origin(t) == Annotated:  # unwrap annotated
        t = t.__origin__
    if hasattr(t, "__origin__"):
        parameters = getattr(t.__origin__, "__parameters__", ())
        args = t.__args__
        t = t.__origin__
    # UnionType has no __origin__
    elif UnionType and isinstance(t, UnionType):  # type: ignore[truthy-function]
        args = t.__args__
        t = Union
    elif hasattr(t, "__parameters__"):
        # Если передан generic-класс без type-параметров, значит по PEP-484 заменяем все параметры на Any
        parameters = t.__parameters__
        args = (Any,) * len(parameters)

    generics = dict(zip(parameters, args))
    filed_format = _find_metadata(metadata, FiledFormat, NoFormat)
    none_format = _find_metadata(metadata, NoneFormat, KeepNone)
    annotation_wrapper = _wrap_annotated([filed_format, none_format])

    if (t, filed_format, none_format) in state:
        return RecursionHolder(
            cls=t,
            name=_generate_name(t, filed_format, none_format),
            field_format=filed_format,
            none_format=none_format,
            state=state,
        )

    if t is Any:
        return AnyType()

    if isinstance(t, type):
        simple_type_mapping: Mapping[type, type[Type]] = {
            bytes: BytesType,
            bool: BooleanType,
            date: DateType,
            time: TimeType,
            datetime: DateTimeType,
            UUID: UUIDType,
        }

        if simple := simple_type_mapping.get(t):
            return simple()

        number_type_mapping: Mapping[type, type[IntegerType] | type[FloatType]] = {
            int: IntegerType,
            float: FloatType,
        }

        if number_type := number_type_mapping.get(t):
            min_meta = _find_metadata(metadata, Min)
            max_meta = _find_metadata(metadata, Max)
            return number_type(
                min=cast(Any, min_meta.value) if min_meta else None,
                max=cast(Any, max_meta.value) if max_meta else None,
            )

        if t is Decimal:
            min_meta = _find_metadata(metadata, Min)
            max_meta = _find_metadata(metadata, Max)
            places_meta = _find_metadata(metadata, Places)
            return DecimalType(
                min=cast(Decimal, min_meta.value) if min_meta else None,
                max=cast(Decimal, max_meta.value) if max_meta else None,
                places=places_meta.value if places_meta else None,
            )

        if t is str:
            min_length_meta = _find_metadata(metadata, MinLength)
            max_length_meta = _find_metadata(metadata, MaxLength)
            return StringType(
                min_length=min_length_meta.value if min_length_meta else None,
                max_length=max_length_meta.value if max_length_meta else None,
            )

        if t in {Sequence, list}:
            return ArrayType(
                item_type=(describe_type(annotation_wrapper(args[0]), state) if args else AnyType()),
                is_sequence=t is Sequence,
            )

        if t in {Mapping, dict}:
            return DictionaryType(
                key_type=(describe_type(annotation_wrapper(args[0]), state) if args else AnyType()),
                value_type=(describe_type(annotation_wrapper(args[1]), state) if args else AnyType()),
                is_mapping=t is Mapping,
                omit_none=none_format.omit,
            )

        if t is tuple:
            if not args or Ellipsis in args:
                raise RuntimeError("Variable length tuples are not supported")
            return TupleType(item_types=[describe_type(annotation_wrapper(arg), state) for arg in args])

        if issubclass(t, (Enum, IntEnum)):
            return EnumType(cls=t)

        if dataclasses.is_dataclass(t):
            state[(t, filed_format, none_format)] = None
            entity_type = _describe_dataclass(t, generics, filed_format, none_format, state)
            state[(t, filed_format, none_format)] = entity_type
            return entity_type

        if attr and attr.has(t):
            state[(t, filed_format, none_format)] = None
            entity_type = _describe_attrs(t, generics, filed_format, none_format, state)
            state[(t, filed_format, none_format)] = entity_type
            return entity_type

    if t in {Union}:
        if len(args) != 2 or _NoneType not in args:
            raise RuntimeError(f"Only Unions of one type with None are supported: {t}, {args}")
        inner = args[1] if args[0] is _NoneType else args[0]
        return OptionalType(describe_type(annotation_wrapper(inner), state))

    if isinstance(t, TypeVar):
        raise RuntimeError(f"Unfilled TypeVar: {t}")

    raise RuntimeError(f"Unknown type {t!r}")


def _describe_dataclass(
    t: type[Any],
    generics: Mapping[TypeVar, Any],
    cls_filed_format: FiledFormat,
    cls_none_format: NoneFormat,
    state: dict[tuple[type, FiledFormat, NoneFormat], Optional[Type]],
) -> EntityType:
    docs = get_attributes_doc(t)
    try:
        types = get_type_hints(t, include_extras=True)
    except Exception:  # pylint: disable=broad-except
        types = {}

    fields = []
    for field in dataclasses.fields(t):
        type_ = _replace_generics(types.get(field.name, field.type), generics)
        type_ = Annotated[type_, cls_filed_format, cls_none_format]

        field_type = describe_type(type_, state)
        metadata = _get_annotated_metadata(type_)
        alias = _find_metadata(metadata, Alias)

        fields.append(
            EntityField(
                name=field.name,
                dict_key=alias.value if alias else _apply_format(cls_filed_format, field.name),
                doc=docs.get(field.name),
                type=field_type,
                default=(field.default if field.default is not dataclasses.MISSING else NOT_SET),
                default_factory=(
                    field.default_factory if field.default_factory is not dataclasses.MISSING else NOT_SET
                ),
                is_property=False,
            )
        )

    return EntityType(
        cls=t,
        name=_generate_name(t, cls_filed_format, cls_none_format),
        fields=fields,
        omit_none=cls_none_format is OmitNone,
        generics=generics,
        doc=t.__doc__,
    )


def _describe_attrs(
    t: type[Any],
    generics: Mapping[TypeVar, Any],
    cls_filed_format: FiledFormat,
    cls_none_format: NoneFormat,
    state: dict[tuple[type, FiledFormat, NoneFormat], Optional[Type]],
) -> EntityType:
    assert attr is not None
    docs = get_attributes_doc(t)
    try:
        types = get_type_hints(t, include_extras=True)
    except Exception:  # pylint: disable=broad-except
        types = {}
    fields = []
    for field in attr.fields(t):  # pyright: ignore
        default = NOT_SET
        if field.default is not attr.NOTHING and not isinstance(field.default, attr.Factory):  # type: ignore[arg-type]
            default = field.default

        default_factory = NOT_SET
        if isinstance(field.default, attr.Factory):  # type: ignore[arg-type]
            default_factory = field.default.factory

        type_ = _replace_generics(types.get(field.name, field.type), generics)
        type_ = Annotated[type_, cls_filed_format, cls_none_format]

        metadata = _get_annotated_metadata(type_)
        field_type = describe_type(type_, state)
        alias = _find_metadata(metadata, Alias)

        fields.append(
            EntityField(
                name=field.name,
                dict_key=alias.value if alias else _apply_format(cls_filed_format, field.name),
                doc=docs.get(field.name),
                type=field_type,
                default=default,
                default_factory=default_factory,
                is_property=False,
            )
        )
    return EntityType(
        cls=t,
        name=_generate_name(t, cls_filed_format, cls_none_format),
        fields=fields,
        omit_none=cls_none_format is OmitNone,
        generics=generics,
    )


def _replace_generics(t: Any, generics: Mapping[TypeVar, Any]) -> Any:
    try:
        if parameters := getattr(t, "__parameters__", None):
            t = t[tuple(generics[parameter] for parameter in parameters)]
        if isinstance(t, TypeVar):
            t = generics[t]
    except KeyError as exc:
        raise RuntimeError(f"Unfilled TypeVar: {exc.args[0]}") from exc
    return t


@overload
def _find_metadata(annotations: Iterable[Any], type_: type[_T], default: _T) -> _T:
    ...


@overload
def _find_metadata(annotations: Iterable[Any], type_: type[_T], default: None = None) -> Optional[_T]:
    ...


def _find_metadata(annotations: Iterable[Any], type_: type[_T], default: Optional[_T] = None) -> Optional[_T]:
    return next((ann for ann in annotations if isinstance(ann, type_)), default)


def _wrap_annotated(annotations: Iterable[Any]) -> Callable[[_T], _T]:
    def inner(type_: _T) -> _T:
        for ann in annotations:
            type_ = Annotated[type_, ann]  # type: ignore
        return type_

    return inner


def _get_annotated_metadata(t: Any) -> tuple[Any, ...]:
    if get_origin(t) == Annotated:
        return getattr(t, "__metadata__", ())
    return ()


def _apply_format(f: Optional[FiledFormat], value: str) -> str:
    if not f or f.format is Format.no_format:
        return value
    if f.format is Format.camel_case:
        return to_camelcase(value)
    assert_never(f.format)


def _generate_name(cls: Any, field_format: FiledFormat, none_format: NoneFormat) -> str:
    nones = "omit_nones" if none_format.omit else "keep_nones"
    return f"{cls.__module__}.{cls.__name__}[{field_format.format.value},{nones}]"
