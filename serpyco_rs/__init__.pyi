from typing import Any, Generic, TypeVar, Type

T = TypeVar('T')


class Serializer(Generic[T]):
    def dump(self, value: T) -> Any:
        pass

    def loads(self, data: dict[str, Any]) -> T:
        pass

def make_serializer(py_class: Type[T]) -> Serializer[T]:
    pass
