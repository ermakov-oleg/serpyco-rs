import serpyco_rs

from .base import Dataclass, test_object


_serializer = serpyco_rs.Serializer(Dataclass, codec=serpyco_rs.JSON)

test_object = test_object


def load(data: bytes) -> Dataclass:
    return _serializer.load(data)


def dump(obj: Dataclass) -> bytes:
    return _serializer.dump(obj)
