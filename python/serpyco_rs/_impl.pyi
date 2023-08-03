from typing import Any, Generic, TypeVar

from ._describe import Type

_T = TypeVar('_T')

class ValidationError(Exception):
    message: str

class ErrorItem:
    message: str
    instance_path: str
    schema_path: str

    def __init__(self, message: str, instance_path: str, schema_path: str): ...

class SchemaValidationError(ValidationError):
    errors: list[ErrorItem]

class Serializer(Generic[_T]):
    def __init__(self, py_class: Type, schema: str, pass_through_bytes: bool): ...
    def dump(self, value: _T) -> Any: ...
    def load(self, data: Any, validate: bool) -> _T: ...
    def load_json(self, data: str, validate: bool) -> _T: ...
