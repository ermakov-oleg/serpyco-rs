import abc
from typing import Annotated, Any, Generic, Protocol, TypeVar, Union, cast, overload

from ._describe import describe_type
from ._impl import Serializer as _Serializer
from ._json_schema import get_json_schema
from .metadata import CamelCase, ForceDefaultForOptional, OmitNone

_T = TypeVar('_T', bound=Any)
_D = TypeVar('_D')


class _MultiMapping(Protocol[_T, _D]):

    @abc.abstractmethod
    def __getitem__(self, __key: str) -> _T: ...

    @overload
    @abc.abstractmethod
    def getall(self, key: str) -> list[_T]: ...
    @overload
    @abc.abstractmethod
    def getall(self, key: str, default: _D) -> Union[list[_T], _D]: ...
    @abc.abstractmethod
    def getall(self, key: str, default: _D = ...) -> Union[list[_T], _D]: ...


class Serializer(Generic[_T]):
    def __init__(
        self,
        t: type[_T],
        *,
        camelcase_fields: bool = False,
        omit_none: bool = False,
        force_default_for_optional: bool = False,
    ) -> None:
        if camelcase_fields:
            t = cast(type[_T], Annotated[t, CamelCase])
        if omit_none:
            t = cast(type(_T), Annotated[t, OmitNone])  # type: ignore
        if force_default_for_optional:
            t = cast(type(_T), Annotated[t, ForceDefaultForOptional])  # type: ignore
        type_info = describe_type(t)
        self._schema = get_json_schema(type_info)
        self._encoder: _Serializer[_T] = _Serializer(type_info)

    def dump(self, value: _T) -> Any:
        return self._encoder.dump(value)

    def load(self, data: Any) -> _T:
        return self._encoder.load(data)

    def load_query_params(self, data: _MultiMapping[Any, Any]) -> _T:
        return self._encoder.load_query_params(data)

    def get_json_schema(self) -> dict[str, Any]:
        return self._schema
