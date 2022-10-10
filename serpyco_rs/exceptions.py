from dataclasses import dataclass

from ._impl import ValidationError

__all__ = ["ValidationError", "SchemaValidationError", "ErrorItem"]


@dataclass
class ErrorItem:
    message: str
    instance_path: str
    schema_path: str


class SchemaValidationError(ValidationError):
    def __init__(self, errors: list[ErrorItem]) -> None:
        self.errors = errors
