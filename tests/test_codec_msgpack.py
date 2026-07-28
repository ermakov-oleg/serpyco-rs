import math
from dataclasses import dataclass
from typing import Any

import pytest

import serpyco_rs
from serpyco_rs import MSGPACK, SchemaValidationError, Serializer, ValidationError


@dataclass
class Payload:
    name: str
    count: int
    flags: list[bool]
    blob: bytes


def test_msgpack_public_codec():
    assert repr(MSGPACK) == '<Codec msgpack>'
    assert serpyco_rs.MSGPACK is MSGPACK


def test_msgpack_roundtrip_with_native_binary():
    value = Payload(name='demo', count=3, flags=[True, False], blob=b'\x00\xff')
    serializer = Serializer(Payload, codec=MSGPACK)
    assert serializer.load(serializer.dump(value)) == value


def test_msgpack_native_binary_wire_type():
    serializer = Serializer(bytes, codec=MSGPACK)
    assert serializer.dump(b'raw') == b'\xc4\x03raw'
    assert serializer.load(b'\xc4\x03raw') == b'raw'
    assert Serializer(Any, codec=MSGPACK).load(b'\xc4\x03raw') == b'raw'


def test_msgpack_loads_standard_compact_encoding():
    # {"name": "x", "values": [1, -1, None]} using fixmap/fixstr/fixarray.
    raw = b'\x82\xa4name\xa1x\xa6values\x93\x01\xff\xc0'
    assert Serializer(dict[str, Any], codec=MSGPACK).load(raw) == {
        'name': 'x',
        'values': [1, -1, None],
    }


def test_msgpack_writer_backpatches_container_lengths():
    raw = Serializer(dict[str, list[int]], codec=MSGPACK).dump({'a': [1, 2, 3]})
    assert raw == b'\xdf\x00\x00\x00\x01\xa1a\xdd\x00\x00\x00\x03\x01\x02\x03'
    assert Serializer(Any, codec=MSGPACK).load(raw) == {'a': [1, 2, 3]}


@pytest.mark.parametrize(
    ('value', 'wire'),
    [
        (127, b'\x7f'),
        (128, b'\xcc\x80'),
        (256, b'\xcd\x01\x00'),
        (2**32, b'\xcf\x00\x00\x00\x01\x00\x00\x00\x00'),
        (-32, b'\xe0'),
        (-33, b'\xd0\xdf'),
        (-(2**63), b'\xd3\x80\x00\x00\x00\x00\x00\x00\x00'),
    ],
)
def test_msgpack_integer_markers(value, wire):
    serializer = Serializer(int, codec=MSGPACK)
    assert serializer.dump(value) == wire
    assert serializer.load(wire) == value


def test_msgpack_uint64_max_roundtrip():
    serializer = Serializer(int, codec=MSGPACK)
    value = 2**64 - 1
    assert serializer.load(serializer.dump(value)) == value


@pytest.mark.parametrize('value', [2**64, -(2**63) - 1])
def test_msgpack_rejects_integers_outside_spec_range(value):
    with pytest.raises(ValidationError, match='out of range for MessagePack'):
        Serializer(int, codec=MSGPACK).dump(value)


@pytest.mark.parametrize('value', [float('nan'), float('inf'), float('-inf')])
def test_msgpack_supports_non_finite_floats(value):
    serializer = Serializer(float, codec=MSGPACK)
    loaded = serializer.load(serializer.dump(value))
    if math.isnan(value):
        assert math.isnan(loaded)
    else:
        assert loaded == value


@pytest.mark.parametrize(
    'raw',
    [
        b'',
        b'\xc1',
        b'\xd9\x03ab',
        b'\xc4\x03ab',
        b'\x92\x01',
        b'\x81\xa1a',
        b'\xa1\xff',
        b'\xc7\x00\x01',
    ],
)
def test_malformed_msgpack_raises_decode_error(raw):
    with pytest.raises(serpyco_rs.DecodeError) as exc:
        Serializer(Any, codec=MSGPACK).load(raw)
    assert isinstance(exc.value.position, int)


def test_msgpack_rejects_trailing_data():
    serializer = Serializer(int, codec=MSGPACK)
    with pytest.raises(serpyco_rs.DecodeError, match='trailing data'):
        serializer.load(b'\x01\x02')


def test_msgpack_schema_error_keeps_path():
    serializer = Serializer(Payload, codec=MSGPACK)
    raw = Serializer(Any, codec=MSGPACK).dump({'name': 'demo', 'count': 'bad', 'flags': [], 'blob': b'x'})
    with pytest.raises(SchemaValidationError) as exc:
        serializer.load(raw)
    assert exc.value.errors[0].instance_path == 'count'


def test_msgpack_requires_string_map_keys():
    # {1: "value"} is legal MessagePack, but serpyco-rs' shared object model
    # requires string keys (matching JSON and aliases/entity field names).
    with pytest.raises(serpyco_rs.DecodeError, match='map keys must be strings'):
        Serializer(Any, codec=MSGPACK).load(b'\x81\x01\xa5value')
