from dataclasses import dataclass
from typing import Optional
from unittest.mock import ANY

from serpyco_rs import Serializer
from serpyco_rs._describe import EntityField, EntityType, OptionalType, RecursionHolder, StringType, describe_type
from serpyco_rs.metadata import NoFormat


@dataclass
class Node:
    value: str
    next: Optional["Node"] = None


@dataclass
class Root:
    head: Node


def test_describe__recursive_type__parsed():
    assert describe_type(Root) == EntityType(
        cls=Root,
        name=ANY,
        fields=[
            EntityField(
                name="head",
                dict_key="head",
                type=EntityType(
                    cls=Node,
                    name=ANY,
                    fields=[
                        EntityField(
                            name="value",
                            dict_key="value",
                            type=StringType(custom_encoder=None),
                        ),
                        EntityField(
                            name="next",
                            dict_key="next",
                            default=None,
                            type=OptionalType(
                                inner=RecursionHolder(
                                    cls=Node,
                                    name=ANY,
                                    field_format=NoFormat,
                                    meta=ANY,
                                    custom_encoder=None,
                                ),
                                custom_encoder=None,
                            ),
                        ),
                    ],
                    doc=ANY,
                    custom_encoder=None,
                ),
            )
        ],
        doc=ANY,
        custom_encoder=None,
    )


def test_serializer():
    serializer = Serializer(Root)

    linked_list = Root(
        head=Node(
            value="1",
            next=Node(value="2"),
        ),
    )

    assert serializer.dump(linked_list) == {"head": {"next": {"next": None, "value": "2"}, "value": "1"}}
    assert serializer.load({"head": {"next": {"next": None, "value": "2"}, "value": "1"}}) == linked_list


@dataclass
class Foo:
    value: str
    next: Optional[list["Foo"]] = None


def test_self_recursive_objects_forward_ref():
    serializer = Serializer(Foo)
    val = Foo(value="a", next=[Foo(value="b")])
    raw = {"value": "a", "next": [{"next": None, "value": "b"}]}
    assert serializer.dump(val) == raw
    assert serializer.load(raw) == val
