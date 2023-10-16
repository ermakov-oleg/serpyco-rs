from collections.abc import Callable
from dataclasses import dataclass
from decimal import Decimal
from enum import Enum
from typing import TypeVar, Union

from ._impl import CustomEncoder


@dataclass(frozen=True)
class Min:
    value: Union[int, float, Decimal]


@dataclass(frozen=True)
class Max:
    value: Union[int, float, Decimal]


@dataclass(frozen=True)
class MinLength:
    value: int


@dataclass(frozen=True)
class MaxLength:
    value: int


@dataclass(frozen=True)
class Discriminator:
    name: str


@dataclass(frozen=True)
class Alias:
    value: str


class Format(Enum):
    no_format = 'no_format'
    camel_case = 'camel_case'


@dataclass(frozen=True)
class FiledFormat:
    format: Format


CamelCase: FiledFormat = FiledFormat(Format.camel_case)
NoFormat: FiledFormat = FiledFormat(Format.no_format)


@dataclass(frozen=True)
class NoneFormat:
    omit: bool


KeepNone: NoneFormat = NoneFormat(False)
OmitNone: NoneFormat = NoneFormat(True)


@dataclass(frozen=True)
class NoneAsDefaultForOptional:
    use: bool


ForceDefaultForOptional: NoneAsDefaultForOptional = NoneAsDefaultForOptional(True)
KeepDefaultForOptional: NoneAsDefaultForOptional = NoneAsDefaultForOptional(False)


_I = TypeVar('_I')
_O = TypeVar('_O')


def serialize_with(func: Callable[[_I], _O]) -> CustomEncoder[_I, _O]:
    return CustomEncoder[_I, _O](serialize=func)


def deserialize_with(func: Callable[[_O], _I]) -> CustomEncoder[_I, _O]:
    return CustomEncoder[_I, _O](deserialize=func)
