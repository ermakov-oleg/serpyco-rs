import serpyco_rs

from . import serpyco_rs_codec
from .serpyco_rs import Issue


_serializer = serpyco_rs.Serializer(Issue, codec=serpyco_rs.MSGPACK)


def load(data: bytes) -> Issue:
    return _serializer.load(data)


def dump(obj: Issue) -> bytes:
    return _serializer.dump(obj)


def from_json(data: bytes) -> bytes:
    """Convert the JSON fixture to this contender's MessagePack wire form."""
    return dump(serpyco_rs_codec.load(data))
