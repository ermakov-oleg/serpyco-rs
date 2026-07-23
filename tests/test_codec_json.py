import pytest

import serpyco_rs


def test_decode_error_exported():
    assert issubclass(serpyco_rs.DecodeError, ValueError)


import uuid
from dataclasses import dataclass, field
from datetime import date, datetime, time, timezone
from decimal import Decimal
from enum import Enum
from typing import Any, Optional

import orjson

from serpyco_rs import JSON, Serializer


class Color(Enum):
    RED = 'red'
    GREEN = 'green'


@dataclass
class Inner:
    name: str
    score: float


@dataclass
class Everything:
    an_int: int
    a_str: str
    a_float: float
    a_bool: bool
    a_none: Optional[int]
    a_list: list[int]
    a_dict: dict[str, int]
    a_tuple: tuple[int, str]
    a_decimal: Decimal
    a_uuid: uuid.UUID
    a_date: date
    a_time: time
    a_datetime: datetime
    an_enum: Color
    an_any: Any
    nested: Inner
    with_default: int = 5
    items: list[Inner] = field(default_factory=list)


EVERYTHING = Everything(
    an_int=42,
    a_str='hello "world"\n',
    a_float=1.5,
    a_bool=True,
    a_none=None,
    a_list=[1, 2, 3],
    a_dict={'a': 1},
    a_tuple=(1, 'x'),
    a_decimal=Decimal('1.100'),
    a_uuid=uuid.UUID('12345678-1234-5678-1234-567812345678'),
    a_date=date(2026, 7, 23),
    a_time=time(12, 30, 0),
    a_datetime=datetime(2026, 7, 23, 12, 30, 0, tzinfo=timezone.utc),
    an_enum=Color.RED,
    an_any={'nested': [1, 'two', None, {'deep': True}]},
    nested=Inner(name='inner', score=0.5),
    items=[Inner(name='a', score=1.0), Inner(name='b', score=2.0)],
)


def test_dump_codec_roundtrip():
    s = Serializer(Everything)
    data = s.dump(EVERYTHING, codec=JSON)
    assert isinstance(data, bytes)
    assert s.load(data, codec=JSON) == EVERYTHING


def test_dump_codec_matches_dict_path():
    s = Serializer(Everything)
    assert orjson.loads(s.dump(EVERYTHING, codec=JSON)) == s.dump(EVERYTHING)


def test_codec_in_constructor():
    s = Serializer(Everything, codec=JSON)
    data = s.dump(EVERYTHING)
    assert isinstance(data, bytes)
    assert s.load(data) == EVERYTHING


def test_load_codec_accepts_str_bytearray_memoryview():
    s = Serializer(Inner, codec=JSON)
    raw = s.dump(Inner(name='x', score=1.0))
    for variant in (raw, raw.decode(), bytearray(raw), memoryview(raw)):
        assert s.load(variant) == Inner(name='x', score=1.0)


def test_per_call_codec_overrides_instance():
    s = Serializer(Inner)
    assert isinstance(s.dump(Inner(name='x', score=1.0), codec=JSON), bytes)
    assert isinstance(s.dump(Inner(name='x', score=1.0)), dict)


def test_big_int_roundtrip():
    s = Serializer(int, codec=JSON)
    big = 2**100
    assert s.load(s.dump(big)) == big
    assert orjson.loads(s.dump(big)) == big


def test_malformed_json_raises_decode_error():
    import serpyco_rs

    s = Serializer(Inner, codec=JSON)
    for bad in (b'{', b'', b'{"name": "x", "score": }', b'[1,]'):
        with pytest.raises(serpyco_rs.DecodeError):
            s.load(bad)


def test_trailing_garbage_on_valid_doc_raises_decode_error():
    s = Serializer(Inner, codec=JSON)
    valid = s.dump(Inner(name='x', score=1.0))
    with pytest.raises(serpyco_rs.DecodeError):
        s.load(valid + b' trailing')


def test_schema_invalid_wellformed_raises_schema_error():
    from serpyco_rs import SchemaValidationError

    s = Serializer(Inner, codec=JSON)
    with pytest.raises(SchemaValidationError):
        s.load(b'{"a": 1}')


def test_schema_error_has_same_instance_path():
    from serpyco_rs import SchemaValidationError

    s = Serializer(Everything)
    good = s.dump(EVERYTHING)
    good['nested'] = {'name': 'x', 'score': 'not a float'}
    import pytest

    with pytest.raises(SchemaValidationError) as dict_err:
        s.load(good)
    with pytest.raises(SchemaValidationError) as codec_err:
        s.load(orjson.dumps(good), codec=JSON)
    assert [(e.message, e.instance_path) for e in codec_err.value.errors] == [
        (e.message, e.instance_path) for e in dict_err.value.errors
    ]


def test_bytes_field_json_dump_raises():
    from serpyco_rs import ValidationError
    import pytest

    s = Serializer(bytes, codec=JSON)
    with pytest.raises(ValidationError):
        s.dump(b'raw')


def test_nan_raises():
    from serpyco_rs import ValidationError
    import pytest

    s = Serializer(float, codec=JSON)
    with pytest.raises(ValidationError):
        s.dump(float('nan'))
