import serpyco_rs

from .serpyco_rs import Issue


_serializer = serpyco_rs.Serializer(Issue, codec=serpyco_rs.JSON)


def load(data: bytes) -> Issue:
    return _serializer.load(data)


def dump(obj: Issue) -> bytes:
    return _serializer.dump(obj)
