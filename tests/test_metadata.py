from dataclasses import dataclass
from typing import Annotated
from decimal import Decimal

import pytest
from serpyco_rs import Serializer
from serpyco_rs.metadata import Alias, Places


def test_annotated_filed_alias():
    @dataclass
    class A:
        foo: Annotated[str, Alias('bar')]

    serializer = Serializer(A)

    obj = A(foo='123')

    expected = {"bar": '123'}
    assert serializer.dump(obj) == expected
    assert serializer.load(expected) == obj
