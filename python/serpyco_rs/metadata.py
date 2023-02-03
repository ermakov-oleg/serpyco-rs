from dataclasses import dataclass
from decimal import Decimal
from enum import Enum
from typing import Union


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
class Places:
    value: int


@dataclass(frozen=True)
class Alias:
    value: str


class Format(Enum):
    no_format = "no_format"
    camel_case = "camel_case"


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
