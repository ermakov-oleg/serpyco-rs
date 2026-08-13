import abc
from collections.abc import Callable
from typing import Annotated, Any, Final, Generic, Protocol, TypeVar, cast, overload

from typing_extensions import TypeForm, TypeVar as _TypeVarExt

from ._custom_types import CustomType
from ._describe import describe_type
from ._impl import Serializer as _Serializer
from ._json_schema import get_json_schema
from ._type_info import BaseType
from .metadata import CamelCase, ForceDefaultForOptional, OmitNone


_T = TypeVar('_T', bound=Any)
_D = TypeVar('_D')


class _MultiMapping(Protocol[_T, _D]):
    """Protocol for a multi-mapping type."""

    @abc.abstractmethod
    def __getitem__(self, key: str, /) -> _T: ...

    @overload
    @abc.abstractmethod
    def getall(self, key: str) -> list[_T]: ...
    @overload
    @abc.abstractmethod
    def getall(self, key: str, default: _D) -> list[_T] | _D: ...
    @abc.abstractmethod
    def getall(self, key: str, default: _D = ...) -> list[_T] | _D: ...


class Codec:
    """Marker for a byte-oriented serialization format.

    Not an extension point: format ids are resolved by the Rust core, so only the
    codecs shipped here can work. Subclassing from outside is rejected at class
    definition rather than failing later at the first `dump`.
    """

    _format_id: int
    _name: str

    def __init_subclass__(cls, **kwargs: Any) -> None:
        if cls.__module__ != __name__:
            raise TypeError(
                f'Codec is not an extension point: {cls.__qualname__} cannot subclass it. '
                f'Formats are defined by the Rust core; use one of the provided codecs (e.g. JSON or MSGPACK).'
            )
        super().__init_subclass__(**kwargs)

    def __repr__(self) -> str:
        return f'<Codec {self._name}>'


class Json(Codec):
    _format_id = 0
    _name = 'json'


JSON: Final[Json] = Json()


class Msgpack(Codec):
    _format_id = 1
    _name = 'msgpack'


MSGPACK: Final[Msgpack] = Msgpack()

_CodecT = _TypeVarExt('_CodecT', bound='Codec | None', default=None)


class Serializer(Generic[_T, _CodecT]):
    _type_info: BaseType

    def __init__(
        self,
        t: TypeForm[_T],
        *,
        camelcase_fields: bool = False,
        omit_none: bool = False,
        force_default_for_optional: bool = False,
        naive_datetime_to_utc: bool = False,
        custom_type_resolver: Callable[[Any], CustomType[Any, Any] | None] | None = None,
        codec: _CodecT = None,  # type: ignore[assignment]
        max_recursion_depth: int = 1000,
    ) -> None:
        """
        Create a serializer for the given type.

        :param t: The type to serialize/deserialize.
        :param camelcase_fields: If True, the serializer will convert field names to camelCase.
        :param omit_none: If True, the serializer will omit None values from the output.
        :param force_default_for_optional: If True, the serializer will force default values for optional fields.
        :param naive_datetime_to_utc: If True, the serializer will convert naive datetimes to UTC.
        :param custom_type_resolver: An optional callable that allows users to add support for their own types.
            This parameter should be a function that takes a type as input and returns an instance of CustomType
            if the user-defined type is supported, or None otherwise.
        :param codec: An optional byte-oriented format (e.g. `JSON` or `MSGPACK`) that binds this serializer to
            `dump`/`load` directly to/from bytes. When set, `dump` returns `bytes` and `load` accepts
            `bytes`/`bytearray`/`memoryview`/`str`. A per-call `codec=` argument then selects a
            different format, but cannot switch the dict-based path back on (`codec=None` is
            indistinguishable from an omitted argument). To keep both modes on one serializer, leave
            this unset and pass `codec=` per call instead.
        :param max_recursion_depth: Maximum number of nested encoder calls before `dump`/`load` raise
            `RecursionError`. Guards against stack overflow on cyclic graphs and pathologically deep input.
            Lower this on platforms with a small thread stack (e.g. Windows defaults to ~1 MiB); raise it
            for genuinely deeply-nested schemas on a fatter stack.
        """
        if camelcase_fields:
            t = cast(TypeForm[_T], Annotated[t, CamelCase])
        if omit_none:
            t = cast(TypeForm[_T], Annotated[t, OmitNone])
        if force_default_for_optional:
            t = cast(TypeForm[_T], Annotated[t, ForceDefaultForOptional])
        self._type_info = describe_type(t, custom_type_resolver=custom_type_resolver)
        self._schema = get_json_schema(self._type_info)
        self._codec = codec
        self._encoder: _Serializer[_T] = _Serializer(self._type_info, naive_datetime_to_utc, max_recursion_depth)

    @overload
    def dump(self: 'Serializer[_T, None]', value: _T, *, codec: None = None) -> Any: ...
    @overload
    def dump(self: 'Serializer[_T, None]', value: _T, *, codec: Json) -> bytes: ...
    @overload
    def dump(self: 'Serializer[_T, None]', value: _T, *, codec: Msgpack) -> bytes: ...
    @overload
    def dump(self: 'Serializer[_T, None]', value: _T, *, codec: 'Codec | None') -> Any: ...
    @overload
    def dump(self, value: _T, *, codec: 'Codec | None' = None) -> bytes: ...

    def dump(self, value: _T, *, codec: 'Codec | None' = None) -> Any:
        """Serialize the value: to a JSON-serializable object, or, when a codec
        is bound (constructor or argument), directly to bytes.

        :param value: The value to serialize.
        :param codec: Optional per-call format (e.g. `JSON` or `MSGPACK`). Selects the format when the serializer
            has none bound, or a different one when it has; it cannot restore the dict-based path.
        """
        active = codec if codec is not None else self._codec
        if active is None:
            return self._encoder.dump(value)
        return self._encoder.dump_bytes(value, active._format_id)

    @overload
    def load(self: 'Serializer[_T, None]', data: Any, *, codec: None = None) -> _T: ...
    @overload
    def load(self: 'Serializer[_T, None]', data: 'bytes | bytearray | memoryview | str', *, codec: Codec) -> _T: ...
    @overload
    def load(self: 'Serializer[_T, None]', data: Any, *, codec: 'Codec | None') -> _T: ...
    @overload
    def load(self, data: 'bytes | bytearray | memoryview | str', *, codec: 'Codec | None' = None) -> _T: ...

    def load(self, data: Any, *, codec: 'Codec | None' = None) -> _T:
        """Deserialize: from a JSON-like object, or, when a codec is bound
        (constructor or argument), directly from bytes.

        :param data: The data to deserialize.
        :param codec: Optional per-call format (e.g. `JSON` or `MSGPACK`). Selects the format when the serializer
            has none bound, or a different one when it has; it cannot restore the dict-based path.
        """
        active = codec if codec is not None else self._codec
        if active is None:
            return self._encoder.load(data)
        return self._encoder.load_bytes(data, active._format_id)

    def load_query_params(self, data: _MultiMapping[Any, Any]) -> _T:
        """Deserialize the given query parameters to the target type.

        :param data: The query parameters to deserialize.
        """
        return self._encoder.load_query_params(data)

    def get_json_schema(self) -> dict[str, Any]:
        """Get the JSON schema for the target type."""
        return self._schema
