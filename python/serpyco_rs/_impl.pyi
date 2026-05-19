from typing import Any, Generic, TypeVar

from ._type_info import BaseType as _BaseType

_T = TypeVar('_T')

class ValidationError(Exception):
    message: str

class ErrorItem:
    message: str
    instance_path: str

    def __init__(self, message: str, instance_path: str) -> None: ...

class SchemaValidationError(ValidationError):
    errors: list[ErrorItem]

class Serializer(Generic[_T]):
    def __init__(self, py_class: _BaseType, naive_datetime_to_utc: bool) -> None: ...
    def dump(self, value: _T) -> Any: ...
    def load(self, data: Any) -> _T: ...
    def load_query_params(self, data: Any) -> _T: ...
