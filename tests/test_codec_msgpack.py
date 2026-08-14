import math
from dataclasses import dataclass
from typing import Any, Optional

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


def test_msgpack_writer_compact_headers_for_known_lengths():
    # Container lengths known up front use the minimal header form —
    # byte-identical to canonical encoders like msgpack-python.
    raw = Serializer(dict[str, list[int]], codec=MSGPACK).dump({'a': [1, 2, 3]})
    assert raw == b'\x81\xa1a\x93\x01\x02\x03'
    assert Serializer(Any, codec=MSGPACK).load(raw) == {'a': [1, 2, 3]}


def test_msgpack_writer_backpatches_dynamic_lengths():
    @dataclass
    class WithOptional:
        a: int
        b: Optional[int] = None

    # omit_none makes the entry count depend on the values, so the writer
    # reserves the 32-bit header form and backpatches the count on close.
    s = Serializer(WithOptional, codec=MSGPACK, omit_none=True)
    assert s.dump(WithOptional(a=1, b=None)) == b'\xdf\x00\x00\x00\x01\xa1a\x01'
    assert s.dump(WithOptional(a=1, b=2)) == b'\xdf\x00\x00\x00\x02\xa1a\x01\xa1b\x02'
    assert s.load(s.dump(WithOptional(a=1, b=None))) == WithOptional(a=1, b=None)


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


@pytest.mark.parametrize('tp', [Any, list[int], dict[str, int]])
@pytest.mark.parametrize(
    'raw',
    [
        b'\xdd\xff\xff\xff\xff',  # array32 claiming 2**32-1 entries, no payload
        b'\xdf\xff\xff\xff\xff',  # map32, same
    ],
)
def test_msgpack_oversized_container_header_is_not_preallocated(tp, raw):
    # The header count sizes the Python container up front, so a claim the input cannot
    # back must be capped by the bytes left — reserving 2**32-1 entries would abort the
    # process long before the truncated payload is reported.
    with pytest.raises(serpyco_rs.DecodeError):
        Serializer(tp, codec=MSGPACK).load(raw)


@pytest.mark.parametrize('size', [0, 1, 15, 16, 100, 65535, 65536])
def test_msgpack_container_roundtrip_across_header_forms(size):
    # fixarray/fixmap, array16/map16 and array32/map32 state their length differently;
    # the presize path has to read each form correctly.
    lst = Serializer(list[int], codec=MSGPACK)
    values = list(range(size))
    assert lst.load(lst.dump(values)) == values

    dct = Serializer(dict[str, int], codec=MSGPACK)
    mapping = {str(i): i for i in range(size)}
    assert dct.load(dct.dump(mapping)) == mapping

    any_codec = Serializer(Any, codec=MSGPACK)
    assert any_codec.load(any_codec.dump(mapping)) == mapping


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


def test_msgpack_schema_error_rendering_is_bounded():
    serializer = Serializer(int, codec=MSGPACK)

    # A huge array at the mismatch point must not be echoed into the message.
    big = Serializer(Any, codec=MSGPACK).dump(list(range(100_000)))
    with pytest.raises(SchemaValidationError) as exc:
        serializer.load(big)
    assert len(str(exc.value)) < 5_000

    # A huge string likewise.
    big_str = Serializer(Any, codec=MSGPACK).dump('x' * 1_000_000)
    with pytest.raises(SchemaValidationError) as exc:
        serializer.load(big_str)
    assert len(str(exc.value)) < 5_000

    # Deep nesting must not overflow the stack while rendering the error —
    # including on a 1 MiB stack (Windows main thread on older CPython), which
    # a small-stack worker thread simulates on every platform.
    deep = b'\x91' * 100_000 + b'\x01'
    with pytest.raises(SchemaValidationError):
        serializer.load(deep)

    import threading

    result: list[BaseException] = []

    def _load_on_small_stack():
        try:
            serializer.load(deep)
        except BaseException as exc:  # noqa: BLE001 - re-raised in the main thread
            result.append(exc)

    threading.stack_size(1024 * 1024)
    try:
        t = threading.Thread(target=_load_on_small_stack)
        t.start()
        t.join()
    finally:
        threading.stack_size(0)
    assert len(result) == 1
    assert isinstance(result[0], SchemaValidationError)


def test_msgpack_requires_string_map_keys():
    # {1: "value"} is legal MessagePack, but serpyco-rs' shared object model
    # requires string keys (matching JSON and aliases/entity field names).
    with pytest.raises(serpyco_rs.DecodeError, match='map keys must be strings'):
        Serializer(Any, codec=MSGPACK).load(b'\x81\x01\xa5value')
