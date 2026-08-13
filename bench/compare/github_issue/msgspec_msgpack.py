import msgspec

# Aliased so the sibling module named ``msgspec`` never shadows the package.
from . import msgspec as msgspec_issue


def load(data: bytes) -> msgspec_issue.Issue:
    return msgspec.msgpack.decode(data, type=msgspec_issue.Issue)


def dump(obj: msgspec_issue.Issue) -> bytes:
    return msgspec.msgpack.encode(obj)


def from_json(data: bytes) -> bytes:
    """Convert the JSON fixture to this contender's MessagePack wire form."""
    return dump(msgspec_issue.load(data))
