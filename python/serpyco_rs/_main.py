from typing import Any, TypeVar, Generic, Annotated

from ._describe import describe_type
from ._impl import make_encoder
from ._json_schema import JsonschemaRSValidator, to_json_schema, Validator
from .metadata import CamelCase

T = TypeVar("T", bound=Any)


class Serializer(Generic[T]):
    def __init__(
        self,
        t: type[T],
        camelcase_fields: bool = False,
        validator_cls: type[Validator] = JsonschemaRSValidator,
    ) -> None:
        if camelcase_fields:
            t = Annotated[t, CamelCase]
        type_info = describe_type(t)
        self._encoder = make_encoder(type_info)
        self._validator = validator_cls(to_json_schema(type_info).dump())

    def dump(self, value: T) -> Any:
        return self._encoder.dump(value)

    def load(self, data: Any, validate: bool = True) -> T:
        if validate:
            self._validator.validate(data)
        return self._encoder.load(data)
