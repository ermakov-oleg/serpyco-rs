from dataclasses import dataclass
from datetime import datetime
from typing import Annotated

import pytest
from serpyco_rs import Serializer, SchemaValidationError, ErrorItem
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


def test_deserialize_with__validation_error():
    serializer = Serializer(Annotated[int, deserialize_with(int)])
    with pytest.raises(SchemaValidationError) as exc_info:
        serializer.load('foo')

    assert exc_info.value.errors == [
        ErrorItem(message="ValueError: invalid literal for int() with base 10: 'foo'", instance_path='')
    ]
