from dataclasses import dataclass
from typing import Annotated, Optional

import pytest
from serpyco_rs import Serializer
from serpyco_rs.metadata import Alias, OmitNone


def test_annotated_filed_alias():
    @dataclass
    class A:
        foo: Annotated[str, Alias("bar")]

    serializer = Serializer(A)

    obj = A(foo="123")

    expected = {"bar": "123"}
    assert serializer.dump(obj) == expected
    assert serializer.load(expected) == obj


def test_omit_none():
    @dataclass
    class A:
        required_val: Optional[bool]
        optional_val: Optional[bool] = None

    serializer = Serializer(Annotated[A, OmitNone])

    expected = {"required_val": None}
    assert serializer.dump(A(required_val=None)) == expected
    assert serializer.load(expected) == A(required_val=None)


@dataclass
class Bar:
    value: Optional[str] = None


@dataclass
class Foo:
    bar: Optional[Bar] = None


@pytest.mark.parametrize(
    ["omit_none", "foo", "expected"],
    [
        (True, Foo(), {}),
        (True, Foo(Bar()), {"bar": {}}),
        (False, Foo(), {"bar": None}),
        (False, Foo(Bar()), {"bar": {"value": None}}),
    ],
)
def test_propagete__omit_none(omit_none, foo, expected):
    serializer = Serializer(Foo, omit_none=omit_none)
    res = serializer.dump(foo)

    assert res == expected


def test_omit_none_on_dict():
    serializer = Serializer(dict[str, Optional[bool]], omit_none=True)
    assert serializer.dump({"foo": True, "bar": None}) == {"foo": True}
