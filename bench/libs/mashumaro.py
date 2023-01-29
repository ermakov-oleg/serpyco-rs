from dataclasses import dataclass
from typing import Optional, Any

from .base import make_test_object
from mashumaro import DataClassDictMixin


@dataclass
class Nested(DataClassDictMixin):
    """
    A nested type for Dataclass
    """

    name: str


@dataclass
class Dataclass(DataClassDictMixin):
    """
    A Dataclass class
    """

    name: str
    value: int
    f: float
    b: bool
    nest: list[Nested]
    many: list[int]
    option: Optional[str] = None


test_object = make_test_object(Dataclass, Nested)


def load(data: dict[str, Any], validate: bool = True) -> Dataclass:
    return Dataclass.from_dict(data)


def dump(obj: Dataclass) -> dict[str, Any]:
    return obj.to_dict()
