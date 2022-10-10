from decimal import Decimal
from typing import Any
import orjson
from abc import ABCMeta, abstractmethod
from serpyco_rs.exceptions import SchemaValidationError
from serpyco_rs._impl import Validator as _Validator


class Validator(metaclass=ABCMeta):
    @abstractmethod
    def validate(self, data: dict[str, Any]) -> None:
        ...

    @abstractmethod
    def validate_json(self, data: str) -> None:
        ...


class ValicoValidator(Validator):
    def __init__(self, schema: dict[str, Any]) -> None:
        self._validator = _Validator(orjson.dumps(schema).decode("utf-8"))

    def validate(self, data: dict[str, Any]) -> None:
        json_string = orjson.dumps(data, default=_default).decode("utf-8")
        return self.validate_json(json_string)

    def validate_json(self, data: str) -> None:
        err = self._validator.validate(data)
        if err:
            raise SchemaValidationError(orjson.loads(err))


def _default(value) -> str:
    if isinstance(value, Decimal):
        return str(value)
