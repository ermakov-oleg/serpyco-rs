from dataclasses import dataclass
from decimal import Decimal
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
