from typing import Any, Optional

from pydantic import BaseModel

from .base import make_test_object


class Nested(BaseModel):
    name: str


class Dataclass(BaseModel):
    name: str
    value: int
    f: float
    b: bool
    nest: list[Nested]
    many: list[int]
    option: Optional[str] = None


test_object = make_test_object(Dataclass, Nested)


def load(data: dict[str, Any], validate: bool = True) -> Dataclass:
    return Dataclass(**data)


def dump(obj: Dataclass) -> dict[str, Any]:
    return obj.dict()
