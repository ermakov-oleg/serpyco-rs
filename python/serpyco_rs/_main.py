from typing import Annotated, Any, Generic, TypeVar, cast

from ._describe import describe_type
from ._impl import Serializer as _Serializer
from ._json_schema import JsonschemaRSValidator, Validator, get_json_schema
from .metadata import CamelCase, OmitNone

_T = TypeVar("_T", bound=Any)


class Serializer(Generic[_T]):
    def __init__(
        self,
        t: type[_T],
        camelcase_fields: bool = False,
        omit_none: bool = False,
        validator_cls: type[Validator] = JsonschemaRSValidator,
    ) -> None:
        if camelcase_fields:
            t = cast(type[_T], Annotated[t, CamelCase])
        if omit_none:
            t = cast(type(_T), Annotated[t, OmitNone])  # type: ignore
        type_info = describe_type(t)
        self._encoder: _Serializer[_T] = _Serializer(type_info)
        self._schema = get_json_schema(type_info)
        self._validator = validator_cls(self._schema)

    def dump(self, value: _T) -> Any:
        return self._encoder.dump(value)

    def load(self, data: Any, validate: bool = True) -> _T:
        if validate:
            self._validator.validate(data)
        return self._encoder.load(data)
