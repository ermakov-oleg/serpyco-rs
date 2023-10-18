from dataclasses import dataclass
from typing import TypeVar, Any, Sequence, Optional

from serpyco_rs._impl import BaseType
from serpyco_rs.metadata import FiledFormat, NoneFormat, NoneAsDefaultForOptional


@dataclass(frozen=True, unsafe_hash=True)
class MetaStateKey:
    cls: type
    field_format: FiledFormat
    none_format: NoneFormat
    none_as_default_for_optional: NoneAsDefaultForOptional
    generics: Sequence[tuple[TypeVar, Any]]


@dataclass
class Meta:
    globals: dict[str, Any]
    state: dict[MetaStateKey, Optional[BaseType]]
    discriminator_field: Optional[str] = None

    def add_to_state(self, key: MetaStateKey, value: Optional[BaseType]) -> None:
        self.state[key] = value

    def __getitem__(self, item) -> Optional[BaseType]:
        return self.state.get(item)

    def has_in_state(self, key: MetaStateKey) -> bool:
        return key in self.state
