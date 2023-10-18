import dataclasses
from collections.abc import Mapping, Sequence
from typing import (
    Any,
    Optional,
)


@dataclasses.dataclass
class Type:
    custom_encoder: Optional[Any]


@dataclasses.dataclass
class BytesType(Type):
    pass


@dataclasses.dataclass
class LiteralType(Type):
    args: Sequence[str]


@dataclasses.dataclass
class UnionType(Type):
    item_types: Mapping[str, Type]
    dump_discriminator: str
    load_discriminator: str


@dataclasses.dataclass
class AnyType(Type):
    pass


@dataclasses.dataclass
class RecursionHolder(Type):
    # todo: Drop
    name: str
    state_key: Any
    meta: Any

    def get_type(self) -> Type:
        if type_ := self.meta.get_from_state(self.state_key):
            return type_
        raise RuntimeError('Recursive type not resolved')
