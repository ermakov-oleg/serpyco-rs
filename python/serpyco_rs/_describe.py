import dataclasses
import sys
from collections.abc import Callable, Iterable, Mapping, Sequence
from datetime import date, datetime, time
from decimal import Decimal
from enum import Enum, IntEnum
from typing import (
    Annotated,
    Any,
    ForwardRef,
    Generic,
    Literal,
    Optional,
    TypeVar,
    Union,
    cast,
    get_origin,
    get_type_hints,
    overload,
)
from uuid import UUID

from attributes_doc import get_attributes_doc
from typing_extensions import NotRequired, Required, assert_never, get_args, is_typeddict

from ._impl import (
    NOT_SET,
    AnyType,
    ArrayType,
    BaseType,
    BooleanType,
    BytesType,
    CustomEncoder,
    DateTimeType,
    DateType,
    DecimalType,
    DefaultValue,
    DictionaryType,
    EntityField,
    EntityType,
    EnumType,
    FloatType,
    IntegerType,
    LiteralType,
    OptionalType,
    RecursionHolder,
    StringType,
    TimeType,
    TupleType,
    TypedDictType,
    UnionType,
    UUIDType,
)
from ._meta import Meta, MetaStateKey
from ._utils import to_camelcase
from .metadata import (
    Alias,
    Discriminator,
    FieldFormat,
    Format,
    KeepDefaultForOptional,
    KeepNone,
    Max,
    MaxLength,
    Min,
    MinLength,
    NoFormat,
    NoneAsDefaultForOptional,
    NoneFormat,
    OmitNone,
)


if sys.version_info >= (3, 10):  # pragma: no cover
    from types import UnionType as StdUnionType
else:  # pragma: no cover
    StdUnionType = None

try:
    import attr
except ImportError:  # pragma: no cover
    attr = None  # type: ignore


_NoneType = type(None)

_T = TypeVar('_T')


def describe_type(t: Any, meta: Optional[Meta] = None) -> BaseType:
    parameters: tuple[Any, ...] = ()
    args: tuple[Any, ...] = ()
    metadata = _get_annotated_metadata(t)
    if get_origin(t) == Annotated:  # unwrap annotated
        t = t.__origin__
    if get_origin(t) in {Required, NotRequired}:  # unwrap TypedDict special forms
        t = t.__args__[0]
    if hasattr(t, '__origin__'):
        parameters = getattr(t.__origin__, '__parameters__', ())
        args = t.__args__
        t = t.__origin__
    # StdUnionType has no __origin__
    elif StdUnionType and isinstance(t, StdUnionType):  # type: ignore[truthy-function]
        args = t.__args__
        t = Union
    elif hasattr(t, '__parameters__'):
        # Если передан generic-класс без type-параметров, значит по PEP-484 заменяем все параметры на Any
        parameters = t.__parameters__
        args = (Any,) * len(parameters)

    if not meta:
        meta = Meta(globals=_get_globals(t), state={})

    t = _evaluate_forwardref(t, meta)

    generics = tuple((k, v) for k, v in sorted(zip(parameters, args), key=lambda x: repr(x[0])))
    filed_format = _find_metadata(metadata, FieldFormat, NoFormat)
    none_format = _find_metadata(metadata, NoneFormat, KeepNone)
    none_as_default_for_optional = _find_metadata(metadata, NoneAsDefaultForOptional, KeepDefaultForOptional)
    custom_encoder = _find_metadata(metadata, CustomEncoder)
    annotation_wrapper = _wrap_annotated([filed_format, none_format, none_as_default_for_optional])

    meta_key = MetaStateKey(
        cls=t,
        field_format=filed_format,
        none_format=none_format,
        none_as_default_for_optional=none_as_default_for_optional,
        generics=generics,
    )

    if meta.has_in_state(meta_key):
        return RecursionHolder(
            name=_generate_name(t, filed_format, none_format, none_as_default_for_optional, generics),
            state_key=meta_key,
            meta=meta,
            custom_encoder=None,
        )

    if t is Any:
        return AnyType(custom_encoder=custom_encoder)

    if isinstance(t, type):
        simple_type_mapping: Mapping[type, type[BaseType]] = {
            bytes: BytesType,
            bool: BooleanType,
            date: DateType,
            time: TimeType,
            datetime: DateTimeType,
            UUID: UUIDType,
        }

        if simple := simple_type_mapping.get(t):
            return simple(custom_encoder=custom_encoder)

        number_type_mapping: Mapping[type, type[Union[IntegerType, FloatType]]] = {
            int: IntegerType,
            float: FloatType,
        }

        if number_type := number_type_mapping.get(t):
            min_meta = _find_metadata(metadata, Min)
            max_meta = _find_metadata(metadata, Max)
            return number_type(
                min=cast(Any, min_meta.value) if min_meta else None,
                max=cast(Any, max_meta.value) if max_meta else None,
                custom_encoder=custom_encoder,
            )

        if t is Decimal:
            min_meta = _find_metadata(metadata, Min)
            max_meta = _find_metadata(metadata, Max)
            return DecimalType(
                min=min_meta.value if min_meta else None,
                max=max_meta.value if max_meta else None,
                custom_encoder=custom_encoder,
            )

        if t is str:
            min_length_meta = _find_metadata(metadata, MinLength)
            max_length_meta = _find_metadata(metadata, MaxLength)
            return StringType(
                min_length=min_length_meta.value if min_length_meta else None,
                max_length=max_length_meta.value if max_length_meta else None,
                custom_encoder=custom_encoder,
            )

        if t in {Sequence, list}:
            return ArrayType(
                item_type=(describe_type(annotation_wrapper(args[0]), meta) if args else AnyType(custom_encoder=None)),
                custom_encoder=custom_encoder,
            )

        if t in {Mapping, dict}:
            return DictionaryType(
                key_type=(describe_type(annotation_wrapper(args[0]), meta) if args else AnyType(custom_encoder=None)),
                value_type=(describe_type(annotation_wrapper(args[1]), meta) if args else AnyType(custom_encoder=None)),
                omit_none=none_format.omit,
                custom_encoder=custom_encoder,
            )

        if t is tuple:
            if not args or Ellipsis in args:
                raise RuntimeError('Variable length tuples are not supported')
            return TupleType(
                item_types=[describe_type(annotation_wrapper(arg), meta) for arg in args],
                custom_encoder=custom_encoder,
            )

        if issubclass(t, (Enum, IntEnum)):
            return EnumType(cls=t, items=[item for item in t], custom_encoder=custom_encoder)

        if dataclasses.is_dataclass(t) or _is_attrs(t) or is_typeddict(t):
            meta.add_to_state(meta_key, None)
            entity_type = _describe_entity(
                t=t,
                generics=generics,
                cls_filed_format=filed_format,
                cls_none_format=none_format,
                custom_encoder=custom_encoder,
                cls_none_as_default_for_optional=none_as_default_for_optional,
                meta=meta,
            )
            meta.add_to_state(meta_key, entity_type)
            return entity_type

    if _is_literal_type(t):
        if args and all(isinstance(arg, str) for arg in args):
            return LiteralType(args=list(args), custom_encoder=custom_encoder)
        raise RuntimeError('Supported only Literal[str, ...]')

    if t in {Union}:
        if len(args) == 2 and _NoneType in args:
            inner = args[1] if args[0] is _NoneType else args[0]
            return OptionalType(inner=describe_type(annotation_wrapper(inner), meta), custom_encoder=None)

        discriminator = _find_metadata(metadata, Discriminator)
        if not discriminator:
            raise RuntimeError('For support Unions need specify serpyco_rs.metadata.Discriminator')

        if not all(dataclasses.is_dataclass(arg) or _is_attrs(arg) for arg in args):
            raise RuntimeError(
                f'Unions supported only for dataclasses or attrs. Provided: {t}[{",".join(map(str, args))}]'
            )

        meta = dataclasses.replace(meta, discriminator_field=discriminator.name)
        return UnionType(
            item_types={
                _get_discriminator_value(arg, discriminator.name): describe_type(annotation_wrapper(arg), meta)
                for arg in args
            },
            dump_discriminator=discriminator.name,
            load_discriminator=_apply_format(filed_format, discriminator.name),
            custom_encoder=custom_encoder,
        )

    if isinstance(t, TypeVar):
        raise RuntimeError(f'Unfilled TypeVar: {t}')

    raise RuntimeError(f'Unknown type {t!r}')


@dataclasses.dataclass
class _Field(Generic[_T]):
    name: str
    type: type[_T]
    default: Union[DefaultValue[_T], DefaultValue[None]] = NOT_SET
    default_factory: Union[DefaultValue[Callable[[], _T]], DefaultValue[None]] = NOT_SET


def _describe_entity(
    t: Any,
    generics: Sequence[tuple[TypeVar, Any]],
    cls_filed_format: FieldFormat,
    cls_none_format: NoneFormat,
    cls_none_as_default_for_optional: NoneAsDefaultForOptional,
    custom_encoder: Optional[CustomEncoder[Any, Any]],
    meta: Meta,
) -> Union[EntityType, TypedDictType]:
    docs = get_attributes_doc(t)
    try:
        types = get_type_hints(t, include_extras=True)
    except Exception:  # pylint: disable=broad-except
        types = {}

    discriminator_field = meta.discriminator_field
    meta = dataclasses.replace(meta, globals=_get_globals(t), discriminator_field=None)

    fields = []
    for field in _get_entity_fields(t):
        type_ = _replace_generics(types.get(field.name, field.type), generics)
        type_ = Annotated[type_, cls_filed_format, cls_none_format, cls_none_as_default_for_optional]

        metadata = _get_annotated_metadata(type_)
        field_type = describe_type(type_, meta)
        alias = _find_metadata(metadata, Alias)
        none_as_default_for_optional = _find_metadata(metadata, NoneAsDefaultForOptional)

        is_discriminator_field = field.name == discriminator_field
        required = not (field.default != NOT_SET or field.default_factory != NOT_SET) or is_discriminator_field

        default = field.default
        if required and none_as_default_for_optional and none_as_default_for_optional.use:
            default = DefaultValue.some(None)
            required = False

        fields.append(
            EntityField(
                name=field.name,
                dict_key=alias.value if alias else _apply_format(cls_filed_format, field.name),
                doc=docs.get(field.name),
                field_type=field_type,
                default=default,
                default_factory=field.default_factory,
                is_discriminator_field=is_discriminator_field,
                required=required,
            )
        )

    if is_typeddict(t):
        return TypedDictType(
            name=_generate_name(t, cls_filed_format, cls_none_format, cls_none_as_default_for_optional, generics),
            fields=fields,
            omit_none=cls_none_format is OmitNone,
            generics=generics,
            doc=t.__doc__,
            custom_encoder=custom_encoder,
        )

    return EntityType(
        cls=t,
        name=_generate_name(t, cls_filed_format, cls_none_format, cls_none_as_default_for_optional, generics),
        fields=fields,
        omit_none=cls_none_format is OmitNone,
        generics=generics,
        doc=_get_dataclass_doc(t),
        custom_encoder=custom_encoder,
    )


def _get_entity_fields(t: Any) -> Sequence[_Field[Any]]:
    if dataclasses.is_dataclass(t):
        return [
            _Field(
                name=f.name,
                type=f.type,
                default=(DefaultValue.some(f.default) if f.default is not dataclasses.MISSING else NOT_SET),
                default_factory=(
                    DefaultValue.some(f.default_factory) if f.default_factory is not dataclasses.MISSING else NOT_SET
                ),
            )
            for f in dataclasses.fields(t)
        ]
    if is_typeddict(t):
        return [
            _Field(
                name=field_name,
                type=field_type,
                default=NOT_SET if _is_required_in_typeddict(t, field_name) else DefaultValue.some(None),
                default_factory=NOT_SET,
            )
            for field_name, field_type in t.__annotations__.items()
        ]
    if _is_attrs(t):
        assert attr
        return [
            _Field(
                name=f.name,
                type=f.type,
                default=(
                    DefaultValue.some(f.default)
                    if (
                        f.default is not attr.NOTHING
                        and not isinstance(f.default, attr.Factory)  # type: ignore[arg-type]
                    )
                    else NOT_SET
                ),
                default_factory=(
                    DefaultValue.some(f.default.factory)  # pyright: ignore
                    if isinstance(f.default, attr.Factory)  # type: ignore[arg-type]
                    else NOT_SET
                ),
            )
            for f in attr.fields(t)
        ]

    raise RuntimeError(f"Unsupported type '{t}'")


def _replace_generics(t: Any, generics: Sequence[tuple[TypeVar, Any]]) -> Any:
    try:
        generics_map = dict(generics)
        if parameters := getattr(t, '__parameters__', None):
            t = t[tuple(generics_map[parameter] for parameter in parameters)]
        if isinstance(t, TypeVar):
            t = generics_map[t]
    except KeyError as exc:
        raise RuntimeError(f'Unfilled TypeVar: {exc.args[0]}') from exc
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
        return getattr(t, '__metadata__', ())
    return ()


def _apply_format(f: Optional[FieldFormat], value: str) -> str:
    if not f or f.format is Format.no_format:
        return value
    if f.format is Format.camel_case:
        return to_camelcase(value)
    assert_never(f.format)


def _generate_name(
    cls: Any,
    field_format: FieldFormat,
    none_format: NoneFormat,
    cls_none_as_default_for_optional: NoneAsDefaultForOptional,
    generics: Sequence[tuple[TypeVar, Any]],
) -> str:
    name = cls.__name__
    if generics:
        cls = _replace_generics(cls, generics)
        name = repr(cls)
    nones = 'omit_nones' if none_format.omit else 'keep_nones'
    force_none = ',force_none' if cls_none_as_default_for_optional.use else ''
    return f'{cls.__module__}.{name}[{field_format.format.value},{nones}{force_none}]'


def _get_globals(t: Any) -> dict[str, Any]:
    if t.__module__ in sys.modules:
        return sys.modules[t.__module__].__dict__.copy()
    return {}


def _evaluate_forwardref(t: type[_T], meta: Meta) -> type[_T]:
    if not isinstance(t, ForwardRef):
        return t

    return t._evaluate(meta.globals, {}, set())


def _get_discriminator_value(t: Any, name: str) -> str:
    fields = attr.fields(t) if attr and _is_attrs(t) else dataclasses.fields(t)
    for field in fields:
        if field.name == name:
            if _is_str_literal(field.type):
                args = get_args(field.type)
                if len(args) != 1:
                    raise RuntimeError(
                        f'Type {t} has invalid discriminator field "{name}". '
                        f'Discriminator supports only Literal[<str>] with one argument.'
                    )

                return cast(str, args[0])

            raise RuntimeError(
                f'Type {t} has invalid discriminator field "{name}" with type "{field.type!r}". '
                f'Discriminator supports only Literal[<str>].'
            )
    raise RuntimeError(f'Type {t} does not have discriminator field "{name}"')


def _is_str_literal(t: Any) -> bool:
    if _is_literal_type(t):
        args = get_args(t)
        if args and all(isinstance(arg, str) for arg in args):
            return True
    return False


def _is_literal_type(t: Any) -> bool:
    return t is Literal or get_origin(t) is Literal


def _is_attrs(t: Any) -> bool:
    return attr is not None and attr.has(t)


def _is_required_in_typeddict(t: Any, key: str) -> bool:
    if is_typeddict(t):
        if t.__total__:
            return key not in t.__optional_keys__
        return key in t.__required_keys__
    raise RuntimeError(f'Expected TypedDict, got "{t!r}"')


def _get_dataclass_doc(cls: Any) -> Optional[str]:
    """Dataclass has automatically generated docstring, which is not very useful."""
    doc: str = cls.__doc__
    if doc is None or doc.startswith(f'{cls.__name__}('):
        return None
    return doc
