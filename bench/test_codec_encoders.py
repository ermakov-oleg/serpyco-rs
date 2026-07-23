"""Per-encoder benchmarks for the JSON codec path (bytes in/out).

Mirrors ``bench/test_encoders.py`` but binds each ``Serializer`` to the ``JSON``
codec, so ``dump`` produces bytes and ``load`` consumes bytes instead of
JSON-like Python objects.
"""

import enum
import uuid
from dataclasses import dataclass
from datetime import date, datetime, time
from decimal import Decimal
from typing import Optional, Union

from serpyco_rs import JSON, Serializer

from .utils import repeat


def test_dump_simple_types_codec(bench_or_check_refcount):
    serializer = Serializer(float, codec=JSON)
    bench_or_check_refcount.group = 'simple_types (codec)'
    bench_or_check_refcount(repeat(lambda: serializer.dump(1)))


def test_load_simple_types_codec(bench_or_check_refcount):
    serializer = Serializer(int, codec=JSON)
    bench_or_check_refcount.group = 'simple_types (codec)'
    raw = serializer.dump(1)
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))


def test_dump_optional_codec(bench_or_check_refcount):
    serializer = Serializer(Optional[int], codec=JSON)

    bench_or_check_refcount.group = 'optional (codec)'

    def inner():
        repeat(lambda: serializer.dump(1))
        repeat(lambda: serializer.dump(None))

    bench_or_check_refcount(inner)


def test_load_optional_codec(bench_or_check_refcount):
    serializer = Serializer(Optional[int], codec=JSON)

    bench_or_check_refcount.group = 'optional (codec)'

    raw1 = serializer.dump(1)
    raw_none = serializer.dump(None)

    def inner():
        repeat(lambda: serializer.load(raw1))
        repeat(lambda: serializer.load(raw_none))

    bench_or_check_refcount(inner)


def test_dump_list_simple_types_codec(bench_or_check_refcount):
    serializer = Serializer(list[int], codec=JSON)
    bench_or_check_refcount.group = 'list (codec)'
    data = list(range(1000))
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


def test_load_list_simple_types_codec(bench_or_check_refcount):
    serializer = Serializer(list[int], codec=JSON)
    bench_or_check_refcount.group = 'list (codec)'
    data = list(range(1000))
    raw = serializer.dump(data)
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))


def test_dump_small_list_simple_types_codec(bench_or_check_refcount):
    serializer = Serializer(list[int], codec=JSON)
    bench_or_check_refcount.group = 'small_list (codec)'
    data = list(range(10))
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


def test_load_small_list_simple_types_codec(bench_or_check_refcount):
    serializer = Serializer(list[int], codec=JSON)
    bench_or_check_refcount.group = 'small_list (codec)'
    data = list(range(10))
    raw = serializer.dump(data)
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))


def test_dump_tuple_simple_types_codec(bench_or_check_refcount):
    serializer = Serializer(tuple[int, str, bool], codec=JSON)
    bench_or_check_refcount.group = 'tuple (codec)'
    bench_or_check_refcount(repeat(lambda: serializer.dump((123, 'foo', True))))


def test_load_tuple_simple_types_codec(bench_or_check_refcount):
    serializer = Serializer(tuple[int, str, bool], codec=JSON)
    bench_or_check_refcount.group = 'tuple (codec)'
    raw = serializer.dump((123, 'foo', True))
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))


def test_dump_dict_simple_types_codec(bench_or_check_refcount):
    serializer = Serializer(dict[str, int], codec=JSON)
    bench_or_check_refcount.group = 'dict (codec)'
    data = {str(i): i for i in range(1000)}
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


def test_dump_dict_dataclass_value_codec(bench_or_check_refcount):
    @dataclass
    class Foo:
        foo: int

    serializer = Serializer(dict[str, Foo], codec=JSON)
    bench_or_check_refcount.group = 'dict (codec)'
    data = {str(i): Foo(i) for i in range(12)}
    bench_or_check_refcount(repeat(lambda: serializer.dump(data), count=100))


def test_load_dict_simple_types_codec(bench_or_check_refcount):
    serializer = Serializer(dict[str, int], codec=JSON)
    bench_or_check_refcount.group = 'dict (codec)'
    data = {str(i): i for i in range(1000)}
    raw = serializer.dump(data)
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))


def test_dump_uuid_codec(bench_or_check_refcount):
    serializer = Serializer(uuid.UUID, codec=JSON)
    bench_or_check_refcount.group = 'uuid (codec)'
    data = uuid.uuid4()
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


def test_load_uuid_codec(bench_or_check_refcount):
    serializer = Serializer(uuid.UUID, codec=JSON)
    bench_or_check_refcount.group = 'uuid (codec)'
    data = uuid.uuid4()
    raw = serializer.dump(data)
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))


def test_dump_date_codec(bench_or_check_refcount):
    serializer = Serializer(date, codec=JSON)
    bench_or_check_refcount.group = 'date (codec)'
    data = date.today()
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


def test_load_date_codec(bench_or_check_refcount):
    serializer = Serializer(date, codec=JSON)
    bench_or_check_refcount.group = 'date (codec)'
    data = date.today()
    raw = serializer.dump(data)
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))


def test_dump_time_codec(bench_or_check_refcount):
    serializer = Serializer(time, codec=JSON)
    bench_or_check_refcount.group = 'time (codec)'
    data = datetime.now().time()
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


def test_load_time_codec(bench_or_check_refcount):
    serializer = Serializer(time, codec=JSON)
    bench_or_check_refcount.group = 'time (codec)'
    data = datetime.now().time()
    raw = serializer.dump(data)
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))


def test_dump_datetime_codec(bench_or_check_refcount):
    serializer = Serializer(datetime, codec=JSON)
    bench_or_check_refcount.group = 'datetime (codec)'
    data = datetime.now()
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


def test_load_datetime_codec(bench_or_check_refcount):
    serializer = Serializer(datetime, codec=JSON)
    bench_or_check_refcount.group = 'datetime (codec)'
    data = datetime.now()
    raw = serializer.dump(data)
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))


def test_dump_decimal_codec(bench_or_check_refcount):
    serializer = Serializer(Decimal, codec=JSON)
    bench_or_check_refcount.group = 'decimal (codec)'
    data = Decimal('1.3')
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


def test_load_decimal_codec(bench_or_check_refcount):
    serializer = Serializer(Decimal, codec=JSON)
    bench_or_check_refcount.group = 'decimal (codec)'
    data = Decimal('1.3')
    raw = serializer.dump(data)
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))


class FooEunm(enum.Enum):
    foo = 'foo'
    bar = 'bar'


def test_dump_enum_codec(bench_or_check_refcount):
    serializer = Serializer(FooEunm, codec=JSON)
    bench_or_check_refcount.group = 'enum (codec)'
    data = FooEunm.bar
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


def test_load_enum_codec(bench_or_check_refcount):
    serializer = Serializer(FooEunm, codec=JSON)
    bench_or_check_refcount.group = 'enum (codec)'
    data = FooEunm.foo
    raw = serializer.dump(data)
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))


@dataclass
class FooDataclass:
    foo: int
    bar: str


def test_dump_dataclass_codec(bench_or_check_refcount):
    serializer = Serializer(FooDataclass, codec=JSON)
    bench_or_check_refcount.group = 'dataclass (codec)'
    data = FooDataclass(foo=1, bar='2')
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


def test_load_dataclass_codec(bench_or_check_refcount):
    serializer = Serializer(FooDataclass, codec=JSON)
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


def test_dump_recursive_codec(bench_or_check_refcount):
    serializer = Serializer(Root, codec=JSON)
    bench_or_check_refcount.group = 'recursive (codec)'
    data = Root(
        head=Node(
            value='1',
            next=Node(value='2'),
        ),
    )
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


def test_load_recursive_codec(bench_or_check_refcount):
    serializer = Serializer(Root, codec=JSON)
    bench_or_check_refcount.group = 'recursive (codec)'
    data = Root(
        head=Node(
            value='1',
            next=Node(value='2'),
        ),
    )
    raw = serializer.dump(data)
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))


def test_dump_union_codec(bench_or_check_refcount):
    @dataclass
    class Foo:
        foo: int

    serializer = Serializer(Union[int, Foo], codec=JSON)
    data = Foo(foo=1)
    bench_or_check_refcount.group = 'union (codec)'
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


def test_load_union_codec(bench_or_check_refcount):
    @dataclass
    class Foo:
        foo: int

    serializer = Serializer(Union[int, Foo], codec=JSON)
    data = Foo(foo=1)
    bench_or_check_refcount.group = 'union (codec)'
    raw = serializer.dump(data)
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))


def test_load_union_miss_first_codec(bench_or_check_refcount):
    @dataclass
    class Foo:
        foo: int

    # First variant (Foo) always misses for an int — this measures the miss cost.
    serializer = Serializer(Union[Foo, int], codec=JSON)
    data = 42
    bench_or_check_refcount.group = 'union (codec)'
    raw = serializer.dump(data)
    bench_or_check_refcount(repeat(lambda: serializer.load(raw)))
