from dataclasses import dataclass
from typing import Annotated, Literal, Union

import pytest
from serpyco_rs import Serializer
from serpyco_rs.metadata import Discriminator


@dataclass
class Foo:
    val: int
    type: Literal["foo"] = "foo"


@dataclass
class Bar:
    type: Literal["bar"]
    val: str


@dataclass
class Buz:
    type: int
    val: str


@dataclass
class Buzz:
    val: str


@dataclass
class Base:
    childs: list[Annotated[Union[Foo, Bar], Discriminator("type")]]


def test_tagged_union():
    serializer = Serializer(Base)
    val = Base(
        childs=[
            Foo(val=1),
            Bar(type="bar", val="12"),
        ]
    )
    raw = {"childs": [{"type": "foo", "val": 1}, {"type": "bar", "val": "12"}]}
    assert serializer.dump(val) == raw
    assert serializer.load(raw) == val


def test_tagged_union__invalid_discriminator_type():
    @dataclass
    class Inner:
        field: Annotated[Union[Foo, Bar, Buz], Discriminator("type")]

    with pytest.raises(RuntimeError) as exc_info:
        Serializer(Inner)

    assert exc_info.type is RuntimeError
    assert exc_info.value.args[0] == (
        "Type <class 'tests.test_union.Buz'> has invalid discriminator field \"type\" with type \"<class 'int'>\". "
        "Discriminator supports only Literal[<str>]."
    )


def test_tagged_union__union_arg_has_no_discriminator_field():
    @dataclass
    class Inner:
        field: Annotated[Union[Foo, Bar, Buzz], Discriminator("type")]

    with pytest.raises(RuntimeError) as exc_info:
        Serializer(Inner)

    assert exc_info.type is RuntimeError
    assert exc_info.value.args[0] == ("Type <class 'tests.test_union.Buzz'> does not have discriminator field \"type\"")


def test_tagged_union__unsupported_types():
    @dataclass
    class Inner:
        field: Annotated[Union[int, str], Discriminator("type")]

    with pytest.raises(RuntimeError) as exc_info:
        Serializer(Inner)

    assert exc_info.type is RuntimeError
    assert exc_info.value.args[0] == (
        "Unions supported only for dataclasses or attrs. Provided: typing.Union[<class 'int'>,<class 'str'>]"
    )
