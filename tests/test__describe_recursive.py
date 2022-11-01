from dataclasses import dataclass
from unittest.mock import ANY

from serpyco_rs._describe import describe_type, EntityType, RecursionHolder, StringType, EntityField


@dataclass
class Inner:
    value: str
    left: "Inner"


@dataclass
class Root:
    inner: Inner


def test_describe__recursive_type__parsed():
    assert describe_type(Root) == EntityType(
        cls=Root,
        fields=[
            EntityField(
                name="inner",
                dict_key="inner",
                type=EntityType(
                    cls=Inner,
                    fields=[
                        EntityField(
                            name="value",
                            dict_key="value",
                            type=StringType(),
                        ),
                        EntityField(
                            name="left",
                            dict_key="left",
                            type=RecursionHolder(
                                cls=Inner,
                            ),
                        ),
                    ],
                    doc=ANY,
                ),
            )
        ],
        doc=ANY,
    )
