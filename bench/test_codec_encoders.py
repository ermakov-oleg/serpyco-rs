import enum
import uuid
from dataclasses import dataclass
from datetime import date, datetime, time
from decimal import Decimal
from typing import Optional, Union

import pytest

from serpyco_rs import JSON, MSGPACK, Codec, Serializer

from .utils import repeat


# One codec per supported byte format. Adding msgpack later is exactly one entry
# here — every benchmark below is already parametrized over this list, and each
# id is derived from the codec itself, so `[json]` keeps its id and CodSpeed
# history when `[msgpack]` shows up alongside it.
CODECS = [JSON, MSGPACK]


def _codec_id(codec: Codec) -> str:
    return codec._name


parametrize_codec = pytest.mark.parametrize('codec', CODECS, ids=_codec_id)


@parametrize_codec
def test_dump_simple_types(bench_or_check_refcount, codec):
    serializer = Serializer(float, codec=codec)
    bench_or_check_refcount.group = 'simple_types (codec)'
    bench_or_check_refcount(repeat(lambda: serializer.dump(1)))


@parametrize_codec
def test_load_simple_types(bench_or_check_refcount, codec):
    serializer = Serializer(int, codec=codec)
    bench_or_check_refcount.group = 'simple_types (codec)'
    raw = serializer.dump(1)
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))


@parametrize_codec
def test_dump_optional(bench_or_check_refcount, codec):
    serializer = Serializer(Optional[int], codec=codec)

    bench_or_check_refcount.group = 'optional (codec)'

    def inner():
        repeat(lambda: serializer.dump(1))
        repeat(lambda: serializer.dump(None))

    bench_or_check_refcount(inner)


@parametrize_codec
def test_load_optional(bench_or_check_refcount, codec):
    serializer = Serializer(Optional[int], codec=codec)

    bench_or_check_refcount.group = 'optional (codec)'

    raw1 = serializer.dump(1)
    raw_none = serializer.dump(None)

    def inner():
        repeat(lambda: serializer.load(raw1))
        repeat(lambda: serializer.load(raw_none))

    bench_or_check_refcount(inner)


@parametrize_codec
def test_dump_list_simple_types(bench_or_check_refcount, codec):
    serializer = Serializer(list[int], codec=codec)
    bench_or_check_refcount.group = 'list (codec)'
    data = list(range(1000))
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


@parametrize_codec
def test_load_list_simple_types(bench_or_check_refcount, codec):
    serializer = Serializer(list[int], codec=codec)
    bench_or_check_refcount.group = 'list (codec)'
    data = list(range(1000))
    raw = serializer.dump(data)
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))


# Floats hit a dedicated wire path in binary formats (fixed-width f64 vs text),
# so they get their own list case: `list[int]` cannot stand in for it. This also
# feeds float dump/load into the PGO profile (ci-pgo-collect runs this file).
@parametrize_codec
def test_dump_list_float(bench_or_check_refcount, codec):
    serializer = Serializer(list[float], codec=codec)
    bench_or_check_refcount.group = 'list_float (codec)'
    data = [i * 1.5 for i in range(1000)]
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


@parametrize_codec
def test_load_list_float(bench_or_check_refcount, codec):
    serializer = Serializer(list[float], codec=codec)
    bench_or_check_refcount.group = 'list_float (codec)'
    data = [i * 1.5 for i in range(1000)]
    raw = serializer.dump(data)
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))


@parametrize_codec
def test_dump_small_list_simple_types(bench_or_check_refcount, codec):
    serializer = Serializer(list[int], codec=codec)
    bench_or_check_refcount.group = 'small_list (codec)'
    data = list(range(10))
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


@parametrize_codec
def test_load_small_list_simple_types(bench_or_check_refcount, codec):
    serializer = Serializer(list[int], codec=codec)
    bench_or_check_refcount.group = 'small_list (codec)'
    data = list(range(10))
    raw = serializer.dump(data)
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))


@parametrize_codec
def test_dump_tuple_simple_types(bench_or_check_refcount, codec):
    serializer = Serializer(tuple[int, str, bool], codec=codec)
    bench_or_check_refcount.group = 'tuple (codec)'
    bench_or_check_refcount(repeat(lambda: serializer.dump((123, 'foo', True))))


@parametrize_codec
def test_load_tuple_simple_types(bench_or_check_refcount, codec):
    serializer = Serializer(tuple[int, str, bool], codec=codec)
    bench_or_check_refcount.group = 'tuple (codec)'
    raw = serializer.dump((123, 'foo', True))
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))


@parametrize_codec
def test_dump_dict_simple_types(bench_or_check_refcount, codec):
    serializer = Serializer(dict[str, int], codec=codec)
    bench_or_check_refcount.group = 'dict (codec)'
    data = {str(i): i for i in range(1000)}
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


@parametrize_codec
def test_dump_dict_dataclass_value(bench_or_check_refcount, codec):
    @dataclass
    class Foo:
        foo: int

    serializer = Serializer(dict[str, Foo], codec=codec)
    bench_or_check_refcount.group = 'dict (codec)'
    data = {str(i): Foo(i) for i in range(12)}
    bench_or_check_refcount(repeat(lambda: serializer.dump(data), count=100))


@parametrize_codec
def test_load_dict_simple_types(bench_or_check_refcount, codec):
    serializer = Serializer(dict[str, int], codec=codec)
    bench_or_check_refcount.group = 'dict (codec)'
    data = {str(i): i for i in range(1000)}
    raw = serializer.dump(data)
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))


@parametrize_codec
def test_dump_uuid(bench_or_check_refcount, codec):
    serializer = Serializer(uuid.UUID, codec=codec)
    bench_or_check_refcount.group = 'uuid (codec)'
    data = uuid.uuid4()
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


@parametrize_codec
def test_load_uuid(bench_or_check_refcount, codec):
    serializer = Serializer(uuid.UUID, codec=codec)
    bench_or_check_refcount.group = 'uuid (codec)'
    data = uuid.uuid4()
    raw = serializer.dump(data)
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))


@parametrize_codec
def test_dump_date(bench_or_check_refcount, codec):
    serializer = Serializer(date, codec=codec)
    bench_or_check_refcount.group = 'date (codec)'
    data = date.today()
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


@parametrize_codec
def test_load_date(bench_or_check_refcount, codec):
    serializer = Serializer(date, codec=codec)
    bench_or_check_refcount.group = 'date (codec)'
    data = date.today()
    raw = serializer.dump(data)
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))


@parametrize_codec
def test_dump_time(bench_or_check_refcount, codec):
    serializer = Serializer(time, codec=codec)
    bench_or_check_refcount.group = 'time (codec)'
    data = datetime.now().time()
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


@parametrize_codec
def test_load_time(bench_or_check_refcount, codec):
    serializer = Serializer(time, codec=codec)
    bench_or_check_refcount.group = 'time (codec)'
    data = datetime.now().time()
    raw = serializer.dump(data)
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))


@parametrize_codec
def test_dump_datetime(bench_or_check_refcount, codec):
    serializer = Serializer(datetime, codec=codec)
    bench_or_check_refcount.group = 'datetime (codec)'
    data = datetime.now()
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


@parametrize_codec
def test_load_datetime(bench_or_check_refcount, codec):
    serializer = Serializer(datetime, codec=codec)
    bench_or_check_refcount.group = 'datetime (codec)'
    data = datetime.now()
    raw = serializer.dump(data)
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))


@parametrize_codec
def test_dump_decimal(bench_or_check_refcount, codec):
    serializer = Serializer(Decimal, codec=codec)
    bench_or_check_refcount.group = 'decimal (codec)'
    data = Decimal('1.3')
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


@parametrize_codec
def test_load_decimal(bench_or_check_refcount, codec):
    serializer = Serializer(Decimal, codec=codec)
    bench_or_check_refcount.group = 'decimal (codec)'
    data = Decimal('1.3')
    raw = serializer.dump(data)
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))


class FooEunm(enum.Enum):
    foo = 'foo'
    bar = 'bar'


@parametrize_codec
def test_dump_enum(bench_or_check_refcount, codec):
    serializer = Serializer(FooEunm, codec=codec)
    bench_or_check_refcount.group = 'enum (codec)'
    data = FooEunm.bar
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


@parametrize_codec
def test_load_enum(bench_or_check_refcount, codec):
    serializer = Serializer(FooEunm, codec=codec)
    bench_or_check_refcount.group = 'enum (codec)'
    data = FooEunm.foo
    raw = serializer.dump(data)
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))


@dataclass
class FooDataclass:
    foo: int
    bar: str


@parametrize_codec
def test_dump_dataclass(bench_or_check_refcount, codec):
    serializer = Serializer(FooDataclass, codec=codec)
    bench_or_check_refcount.group = 'dataclass (codec)'
    data = FooDataclass(foo=1, bar='2')
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


@parametrize_codec
def test_load_dataclass(bench_or_check_refcount, codec):
    serializer = Serializer(FooDataclass, codec=codec)
    bench_or_check_refcount.group = 'dataclass (codec)'
    data = FooDataclass(foo=1, bar='2')
    raw = serializer.dump(data)
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))


@dataclass
class Node:
    value: str
    next: Optional['Node'] = None


@dataclass
class Root:
    head: Node


@parametrize_codec
def test_dump_recursive(bench_or_check_refcount, codec):
    serializer = Serializer(Root, codec=codec)
    bench_or_check_refcount.group = 'recursive (codec)'
    data = Root(
        head=Node(
            value='1',
            next=Node(value='2'),
        ),
    )
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


@parametrize_codec
def test_load_recursive(bench_or_check_refcount, codec):
    serializer = Serializer(Root, codec=codec)
    bench_or_check_refcount.group = 'recursive (codec)'
    data = Root(
        head=Node(
            value='1',
            next=Node(value='2'),
        ),
    )
    raw = serializer.dump(data)
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))


@parametrize_codec
def test_dump_union(bench_or_check_refcount, codec):
    @dataclass
    class Foo:
        foo: int

    serializer = Serializer(Union[int, Foo], codec=codec)
    data = Foo(foo=1)
    bench_or_check_refcount.group = 'union (codec)'
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


@parametrize_codec
def test_load_union(bench_or_check_refcount, codec):
    @dataclass
    class Foo:
        foo: int

    serializer = Serializer(Union[int, Foo], codec=codec)
    data = Foo(foo=1)
    bench_or_check_refcount.group = 'union (codec)'
    raw = serializer.dump(data)
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))


@parametrize_codec
def test_load_union_miss_first(bench_or_check_refcount, codec):
    @dataclass
    class Foo:
        foo: int

    # First variant (Foo) always misses for an int — this measures the miss cost.
    serializer = Serializer(Union[Foo, int], codec=codec)
    data = 42
    bench_or_check_refcount.group = 'union (codec)'
    raw = serializer.dump(data)
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))
