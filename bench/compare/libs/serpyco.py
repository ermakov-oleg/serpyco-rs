from typing import Any

import serpyco

from .base import Dataclass, test_object


_serializer = serpyco.Serializer(Dataclass)

test_object = test_object


def load(data: dict[str, Any]) -> Dataclass:
    return _serializer.load(data, validate=True)


def dump(obj: Dataclass) -> dict[str, Any]:
    return _serializer.dump(obj)
