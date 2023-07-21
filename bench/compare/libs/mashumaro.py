from dataclasses import dataclass
from typing import Any, Optional

from mashumaro import DataClassDictMixin
from mashumaro.mixins.json import DataClassJSONMixin
from orjson import orjson

from .base import make_test_object


@dataclass
class Nested(DataClassJSONMixin, DataClassDictMixin):
    """
    A nested type for Dataclass
    """

    name: str


@dataclass
class Dataclass(DataClassJSONMixin, DataClassDictMixin):
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


def load_json(data: str, validate: bool = True) -> Dataclass:
    return Dataclass.from_json(data, decoder=orjson.loads)


def dump(obj: Dataclass) -> dict[str, Any]:
    return obj.to_dict()
