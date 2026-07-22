from typing import Any

import msgspec

from .base import Dataclass, test_object


test_object = test_object


def load(data: dict[str, Any]) -> Dataclass:
    return msgspec.convert(data, type=Dataclass)


def dump(obj: Dataclass) -> dict[str, Any]:
    return msgspec.to_builtins(obj)
