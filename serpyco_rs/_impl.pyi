from typing import Any, Generic, TypeVar

from ._describe import Type

T = TypeVar("T")

class ValidationError(Exception):
    pass

class Serializer(Generic[T]):
    def dump(self, value: T) -> Any:
        pass
    def load(self, data: Any) -> T:
        pass

def make_encoder(py_class: Type) -> Serializer[T]:
    pass
