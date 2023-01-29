from dataclasses import dataclass
from typing import Optional, Any

from mashumaro import DataClassDictMixin


@dataclass
class Nested(DataClassDictMixin):
    """
    A nested type for Dataclass
    """

    name: str


@dataclass
class Dataclass(DataClassDictMixin):
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


test_object = Dataclass(
    name="Foo",
    value=42,
    f=12.34,
    b=True,
    nest=[Nested(name="Bar_{}".format(index)) for index in range(0, 1000)],
    many=[1, 2, 3],
)


def load(data: dict[str, Any], validate: bool = True) -> Dataclass:
    return Dataclass.from_dict(data)


def dump(obj: Dataclass) -> dict[str, Any]:
    return obj.to_dict()
