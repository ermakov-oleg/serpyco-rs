from abc import ABCMeta, abstractmethod
from typing import Any

import jsonschema_rs

from serpyco_rs.exceptions import SchemaValidationError, ErrorItem


class Validator(metaclass=ABCMeta):
    @abstractmethod
    def __init__(self, schema: dict[str, Any]) -> None:
        ...

    @abstractmethod
    def validate(self, data: dict[str, Any]) -> None:
        ...


class JsonschemaRSValidator(Validator):
    def __init__(self, schema: dict[str, Any]) -> None:
        self._validator = jsonschema_rs.JSONSchema(schema)

    def validate(self, data: dict[str, Any]) -> None:
        errors = list(self._validator.iter_errors(data))

        if errors:
            raise SchemaValidationError([self._map_err(e) for e in errors])

    def _map_err(self, err: jsonschema_rs.ValidationError) -> ErrorItem:
        return ErrorItem(
            message=err.message,
            instance_path="/".join(err.instance_path),
            schema_path="/".join(err.schema_path),
        )
