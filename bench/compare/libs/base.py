from dataclasses import dataclass
from typing import Optional


@dataclass
class Nested:
    """
    A nested type for Dataclass
    """

    name: str


@dataclass
class Dataclass:
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


def make_test_object(cls, nested_cls):
    return cls(
        name="Foo",
        value=42,
        f=12.34,
        b=True,
        nest=[nested_cls(name="Bar_{}".format(index)) for index in range(0, 1000)],
        many=[1, 2, 3],
    )


test_object = make_test_object(Dataclass, Nested)
