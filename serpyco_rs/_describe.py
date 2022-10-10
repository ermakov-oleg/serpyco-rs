import dataclasses
import sys
from collections.abc import Callable, Iterable, Mapping, Sequence
from datetime import date, datetime, time
from decimal import Decimal
from functools import lru_cache
from typing import TYPE_CHECKING, Any, Optional, TypeVar, Union, cast, get_type_hints
from uuid import UUID
from enum import Enum, IntEnum


from .metadata import Min, Max, MaxLength, MinLength, Places

from attributes_doc import get_attributes_doc


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
    type: Type
    doc: Optional[str] = None
    default: Any = NOT_SET
    default_factory: Union[Callable[[], Any], NotSet] = NOT_SET
    is_property: bool = False


@dataclasses.dataclass
class EntityType(Type):
    cls: type[Any]
    fields: Sequence[EntityField]
    generics: Mapping[TypeVar, Any] = dataclasses.field(default_factory=dict)
    doc: Optional[str] = None
    is_presenter: bool = False


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


@dataclasses.dataclass
class TupleType(Type):
    item_types: Sequence[Type]


@dataclasses.dataclass
class AnyType(Type):
    pass


def describe_type(t: Any) -> Type:
    parameters: tuple[Any, ...] = ()
    args: tuple[Any, ...] = ()
    metadata: tuple[Any, ...] = ()
    if hasattr(t, "__origin__"):
        parameters = getattr(t.__origin__, "__parameters__", ())
        args = t.__args__
        metadata = getattr(t, "__metadata__", ())
        t = t.__origin__
    elif UnionType and isinstance(t, UnionType):  # UnionType has no __origin__
        args = t.__args__
        t = Union
    elif hasattr(t, "__parameters__"):
        # Если передан generic-класс без type-параметров, значит по PEP-484 заменяем все параметры на Any
        parameters = t.__parameters__
        args = (Any,) * len(parameters)

    generics = dict(zip(parameters, args))

    if t is Any:
        return AnyType()

    if isinstance(t, type):

        if simple := {
            bytes: BytesType,
            bool: BooleanType,
            date: DateType,
            time: TimeType,
            datetime: DateTimeType,
            UUID: UUIDType,
        }.get(t):
            return simple()

        if number_type := {
            int: IntegerType,
            float: FloatType,
        }.get(t):
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
                item_type=describe_type(args[0]) if args else AnyType(),
                is_sequence=t is Sequence,
            )

        if t in {Mapping, dict}:
            return DictionaryType(
                key_type=describe_type(args[0]) if args else AnyType(),
                value_type=describe_type(args[1]) if args else AnyType(),
                is_mapping=t is Mapping,
            )

        if t is tuple:
            if not args or Ellipsis in args:
                raise RuntimeError("Variable length tuples are not supported")
            return TupleType(item_types=[describe_type(arg) for arg in args])

        if issubclass(t, (Enum, IntEnum)):
            return EnumType(cls=t)

        if dataclasses.is_dataclass(t):
            return _describe_dataclass(t, generics)

        if attr and attr.has(t):
            return _describe_attrs(t, generics)

    if t in {Union}:
        if len(args) != 2 or _NoneType not in args:
            raise RuntimeError(
                f"Only Unions of one type with None are supported: {t}, {args}"
            )
        inner = args[1] if args[0] is _NoneType else args[0]
        return OptionalType(describe_type(inner))

    if isinstance(t, TypeVar):
        raise RuntimeError(f"Unfilled TypeVar: {t}")

    raise RuntimeError(f"Unknown type {t!r}")


if not TYPE_CHECKING:
    # Mypy считает, что типы unhashable
    describe_type = lru_cache()(describe_type)


def _describe_dataclass(t: type[Any], generics: Mapping[TypeVar, Any]) -> EntityType:
    docs = get_attributes_doc(t)
    try:
        types = get_type_hints(t, include_extras=True)
    except Exception:  # pylint: disable=broad-except
        types = {}

    fields = []
    for field in dataclasses.fields(t):
        fields.append(
            EntityField(
                name=field.name,
                doc=docs.get(field.name),
                type=describe_type(
                    _replace_generics(types.get(field.name, field.type), generics)
                ),
                default=(
                    field.default
                    if field.default is not dataclasses.MISSING
                    else NOT_SET
                ),
                default_factory=(
                    field.default_factory
                    if field.default_factory is not dataclasses.MISSING
                    else NOT_SET
                ),
                is_property=False,
            )
        )

    return EntityType(cls=t, fields=fields, generics=generics, doc=t.__doc__)


def _describe_attrs(t: type[Any], generics: Mapping[TypeVar, Any]) -> EntityType:
    docs = get_attributes_doc(t)
    try:
        types = get_type_hints(t, include_extras=True)
    except Exception:  # pylint: disable=broad-except
        types = {}
    fields = []
    for field in attr.fields(t):
        default = NOT_SET
        if field.default is not attr.NOTHING and not isinstance(field.default, attr.Factory):  # type: ignore[arg-type]
            default = field.default

        default_factory = NOT_SET
        if isinstance(field.default, attr.Factory):  # type: ignore[arg-type]
            default_factory = field.default.factory

        fields.append(
            EntityField(
                name=field.name,
                doc=docs.get(field.name),
                type=describe_type(
                    _replace_generics(types.get(field.name, field.type), generics)
                ),
                default=default,
                default_factory=default_factory,
                is_property=False,
            )
        )
    return EntityType(cls=t, fields=fields, generics=generics)


def _replace_generics(t: Any, generics: Mapping[TypeVar, Any]) -> Any:
    try:
        if parameters := getattr(t, "__parameters__", None):
            t = t[tuple(generics[parameter] for parameter in parameters)]
        if isinstance(t, TypeVar):
            t = generics[t]
    except KeyError as exc:
        raise RuntimeError(f"Unfilled TypeVar: {exc.args[0]}") from exc
    return t


def _find_metadata(annotations: Iterable[Any], type_: type[_T]) -> Optional[_T]:
    return next((ann for ann in annotations if isinstance(ann, type_)), None)
