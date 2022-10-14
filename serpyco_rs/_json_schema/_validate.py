from abc import ABCMeta, abstractmethod
from typing import Any

import jsonschema_rs

from serpyco_rs.exceptions import SchemaValidationError, ErrorItem


class Validator(metaclass=ABCMeta):
    @abstractmethod
    def __init__(self, schema: dict[str, Any]) -> None:
        ...

    @abstractmethod
    def validate(self, data: Any) -> None:
        ...


class JsonschemaRSValidator(Validator):
    def __init__(self, schema: dict[str, Any]) -> None:
        self._validator = jsonschema_rs.JSONSchema(schema)

    def validate(self, data: Any) -> None:
        if not self._validator.is_valid(data):
            errors = list(self._validator.iter_errors(data))
            if errors:
                raise SchemaValidationError([self._map_err(e) for e in errors])

    def _map_err(self, err: jsonschema_rs.ValidationError) -> ErrorItem:
        return ErrorItem(
            message=err.message,
            instance_path="/".join(map(str, err.instance_path)),
            schema_path="/".join(map(str, err.schema_path)),
        )
