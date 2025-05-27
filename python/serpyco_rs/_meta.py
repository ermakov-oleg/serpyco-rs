from dataclasses import dataclass
from typing import Any, Optional, TypeVar, overload

from typing_extensions import Self

from serpyco_rs._impl import BaseType
from serpyco_rs.metadata import FieldFormat, NoneAsDefaultForOptional, NoneFormat


@dataclass
class ResolverContext:
    """Context for the type resolution process"""

    globals: dict[str, Any]
    type_cache: dict[str, Optional[BaseType]]
    discriminator_field: Optional[str] = None

    def cache_type(self, key: str, value: Optional[BaseType]) -> None:
        """Cache a resolved type"""
        self.type_cache[key] = value

    def __getitem__(self, item: str) -> Optional[BaseType]:
        return self.type_cache.get(item)

    def get_cached_type(self, key: str) -> Optional[BaseType]:
        """Get a cached type by key"""
        return self.type_cache.get(key)

    def has_cached_type(self, key: str) -> bool:
        """Check if a type is cached"""
        return key in self.type_cache


_T = TypeVar('_T')

_PROPAGETABLE_METADATA_TYPES = {FieldFormat, NoneAsDefaultForOptional, NoneFormat}


class Annotations:
    """Storage for field annotations from typing.Annotated"""

    _data: dict[type, Any]

    def __init__(self, *args: Any):
        self._data = {}
        for arg in args:
            self._data[type(arg)] = arg

    @overload
    def get(self, key: type[_T], default: None = None) -> Optional[_T]: ...

    @overload
    def get(self, key: type[_T], default: _T) -> _T: ...

    def get(self, key: type[_T], default: Optional[_T] = None) -> Optional[_T]:
        return self._data.get(key, default)

    def merge(self, other: Self) -> Self:
        new_data = self.__class__()
        new_data._data = {
            **{k: v for k, v in self._data.items() if k in _PROPAGETABLE_METADATA_TYPES},
            **other._data,
        }
        return new_data

    def make_key(self) -> str:
        d = list(map(str, self._data.values()))
        d.sort()
        return str(d)

    def __eq__(self, value: object, /) -> bool:
        if isinstance(value, self.__class__):
            return self._data == value._data
        return False

    def __repr__(self) -> str:
        return f'{self.__class__.__name__}({tuple(self._data.values())})'
