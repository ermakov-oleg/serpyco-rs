from typing import Any, Generic, TypeVar

from ._describe import Type

_T = TypeVar("_T")

class ValidationError(Exception):
    pass

class Serializer(Generic[_T]):
    def __init__(self, py_class: Type):
        pass
    def dump(self, value: _T) -> Any:
        pass
    def load(self, data: Any) -> _T:
        pass
