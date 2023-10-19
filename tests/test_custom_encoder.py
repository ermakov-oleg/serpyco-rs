from dataclasses import dataclass
from datetime import datetime
from typing import Annotated

import pytest
from serpyco_rs import Serializer
from serpyco_rs.metadata import CustomEncoder, deserialize_with, serialize_with


@dataclass
class Foo:
    val: Annotated[str, CustomEncoder[str, str](serialize=str.upper, deserialize=str.lower)]


def test_custom_encoder():
    serializer = Serializer(Foo)
    val = Foo(val='foo')
    raw = {'val': 'FOO'}
    assert serializer.dump(val) == raw
    assert serializer.load(raw) == val


def test_serialize_with():
    serializer = Serializer(Annotated[datetime, serialize_with(lambda x: x)])
    val = datetime.now()
    assert serializer.dump(val) is val


def test_deserialize_with():
    serializer = Serializer(Annotated[datetime, deserialize_with(lambda x: x)])
    val = datetime.now()
    assert serializer.load(val) is val
