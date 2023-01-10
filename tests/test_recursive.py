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
                            type=StringType(),
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
                                    state=ANY,
                                ),
                            ),
                        ),
                    ],
                    doc=ANY,
                ),
            )
        ],
        doc=ANY,
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
