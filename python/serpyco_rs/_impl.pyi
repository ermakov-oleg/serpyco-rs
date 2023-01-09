from typing import Any, Generic, TypeVar

from ._describe import Type

_T = TypeVar("_T")

class ValidationError(Exception):
    pass

class Serializer(Generic[_T]):
    def dump(self, value: _T) -> Any:
        pass
    def load(self, data: Any) -> _T:
        pass

def make_encoder(py_class: Type) -> Serializer:
    pass
