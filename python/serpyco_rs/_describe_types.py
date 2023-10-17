import dataclasses
from collections.abc import Mapping, Sequence
from enum import Enum, IntEnum
from typing import (
    Any,
    Optional,
    Union,
)


@dataclasses.dataclass
class Type:
    custom_encoder: Optional[Any]


@dataclasses.dataclass
class BytesType(Type):
    pass


@dataclasses.dataclass
class EnumType(Type):
    cls: type[Union[Enum, IntEnum]]


@dataclasses.dataclass
class LiteralType(Type):
    args: Sequence[str]


@dataclasses.dataclass
class OptionalType(Type):
    inner: Type


@dataclasses.dataclass
class ArrayType(Type):
    item_type: Type
    is_sequence: bool


@dataclasses.dataclass
class DictionaryType(Type):
    key_type: Type
    value_type: Type
    is_mapping: bool
    omit_none: bool = False


@dataclasses.dataclass
class TupleType(Type):
    item_types: Sequence[Type]


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
