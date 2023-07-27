import json
from typing import Annotated, Any, Generic, TypeVar, cast

from ._describe import describe_type
from ._impl import InnerSchemaValidationError, Serializer as _Serializer
from ._json_schema import JsonschemaRSValidator, Validator, get_json_schema
from .exceptions import ErrorItem, SchemaValidationError
from .metadata import CamelCase, ForceDefaultForOptional, OmitNone


_T = TypeVar('_T', bound=Any)


class Serializer(Generic[_T]):
    def __init__(
        self,
        t: type[_T],
        *,
        camelcase_fields: bool = False,
        omit_none: bool = False,
        force_default_for_optional: bool = False,
        validator_cls: type[Validator] = JsonschemaRSValidator,
    ) -> None:
        if camelcase_fields:
            t = cast(type[_T], Annotated[t, CamelCase])
        if omit_none:
            t = cast(type(_T), Annotated[t, OmitNone])  # type: ignore
        if force_default_for_optional:
            t = cast(type(_T), Annotated[t, ForceDefaultForOptional])  # type: ignore
        type_info = describe_type(t)
        self._schema = get_json_schema(type_info)
        self._encoder: _Serializer[_T] = _Serializer(type_info, json.dumps(self._schema))
        self._validator = validator_cls(self._schema)

    def dump(self, value: _T) -> Any:
        return self._encoder.dump(value)

    def load(self, data: Any, validate: bool = True) -> _T:
        if validate:
            self._validator.validate(data)
        return self._encoder.load(data)

    def load_json(self, data: str, validate: bool = True) -> _T:
        try:
            return self._encoder.load_json(data, validate)
        except InnerSchemaValidationError as e:
            items = e.args[0]
            raise SchemaValidationError(
                errors=[
                    ErrorItem(
                        message=item.args[0],
                        schema_path=item.args[1],
                        instance_path=item.args[2],
                    )
                    for item in items
                ]
            ) from None

    def get_json_schema(self) -> dict[str, Any]:
        return self._schema
