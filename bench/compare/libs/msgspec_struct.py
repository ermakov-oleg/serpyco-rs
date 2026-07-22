from typing import Any, Optional

import msgspec

from .base import make_test_object


class Nested(msgspec.Struct):
    """
    A nested type for Dataclass
    """

    name: str


class Dataclass(msgspec.Struct):
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


def load(data: dict[str, Any]) -> Dataclass:
    return msgspec.convert(data, type=Dataclass)


def dump(obj: Dataclass) -> dict[str, Any]:
    return msgspec.to_builtins(obj)
