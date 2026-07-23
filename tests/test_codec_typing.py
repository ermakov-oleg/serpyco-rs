from dataclasses import dataclass
from typing import Any

from serpyco_rs import JSON, Json, Serializer
from typing_extensions import assert_type


@dataclass
class Foo:
    x: int


def check_no_codec(s: Serializer[Foo]) -> None:
    assert_type(s.dump(Foo(x=1)), Any)
    assert_type(s.load({'x': 1}), Foo)
    assert_type(s.dump(Foo(x=1), codec=JSON), bytes)
    assert_type(s.load(b'{"x": 1}', codec=JSON), Foo)


def check_constructor_codec() -> None:
    s = Serializer(Foo, codec=JSON)
    assert_type(s, Serializer[Foo, Json])
    assert_type(s.dump(Foo(x=1)), bytes)
    assert_type(s.load(b'{"x": 1}'), Foo)


def test_smoke() -> None:
    # runtime part: this file is also collected by pytest
    s = Serializer(Foo, codec=JSON)
    assert s.load(s.dump(Foo(x=1))) == Foo(x=1)
