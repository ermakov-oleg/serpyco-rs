from typing import Any

import serpyco_rs

from .base import Dataclass, test_object

_serializer = serpyco_rs.Serializer(Dataclass)

test_object = test_object


def load(data: dict[str, Any], validate: bool = True) -> Dataclass:
    return _serializer.load(data, validate=validate)


def dump(obj: Dataclass) -> dict[str, Any]:
    return _serializer.dump(obj)
