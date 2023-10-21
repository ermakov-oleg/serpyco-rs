import enum
import uuid
from dataclasses import dataclass
from datetime import date, datetime, time
from decimal import Decimal
from typing import Optional

import pytest
from serpyco_rs import Serializer

from .utils import repeat, check_refcount


def test_dump_simple_types(benchmark):
    serializer = Serializer(int)
    benchmark.group = 'simple_types'
    with check_refcount():
        benchmark(repeat(lambda: serializer.dump(1)))


def test_load_simple_types(benchmark):
    serializer = Serializer(int)
    benchmark.group = 'simple_types'
    with check_refcount():
        benchmark(repeat(lambda: serializer.load(1)))


def test_dump_optional(benchmark):
    serializer = Serializer(Optional[int])

    benchmark.group = 'optional'

    with check_refcount():

        @benchmark
        def inner():
            repeat(lambda: serializer.dump(1))
            repeat(lambda: serializer.dump(None))


def test_load_optional(benchmark):
    serializer = Serializer(Optional[int])

    benchmark.group = 'optional'

    with check_refcount():

        @benchmark
        def inner():
            repeat(lambda: serializer.load(1))
            repeat(lambda: serializer.load(None))


def test_dump_list_simple_types(benchmark):
    serializer = Serializer(list[int])
    benchmark.group = 'list'
    data = list(range(1000))
    benchmark(repeat(lambda: serializer.dump(data)))


def test_load_list_simple_types(benchmark):
    serializer = Serializer(list[int])
    benchmark.group = 'list'
    data = list(range(1000))
    with check_refcount():
        benchmark(repeat(lambda: serializer.load(data)))


def test_dump_tuple_simple_types(benchmark):
    serializer = Serializer(tuple[int, str, bool])
    benchmark.group = 'tuple'
    with check_refcount():
        benchmark(repeat(lambda: serializer.dump((123, 'foo', True))))


def test_load_tuple_simple_types(benchmark):
    serializer = Serializer(tuple[int, str, bool])
    benchmark.group = 'tuple'
    with check_refcount():
        benchmark(repeat(lambda: serializer.load((123, 'foo', True))))


def test_dump_dict_simple_types(benchmark):
    serializer = Serializer(dict[str, int])
    benchmark.group = 'dict'
    data = {str(i): i for i in range(1000)}
    with check_refcount():
        benchmark(repeat(lambda: serializer.dump(data)))


@pytest.mark.slowtest
def test_load_dict_simple_types(benchmark):
    serializer = Serializer(dict[str, int])
    benchmark.group = 'dict'
    data = {str(i): i for i in range(1000)}
    with check_refcount():
        benchmark(repeat(lambda: serializer.load(data), count=100))


def test_dump_uuid(benchmark):
    serializer = Serializer(uuid.UUID)
    benchmark.group = 'uuid'
    data = uuid.uuid4()
    with check_refcount():
        benchmark(repeat(lambda: serializer.dump(data)))


def test_load_uuid(benchmark):
    serializer = Serializer(uuid.UUID)
    benchmark.group = 'uuid'
    data = str(uuid.uuid4())
    with check_refcount():
        benchmark(repeat(lambda: serializer.load(data)))


def test_dump_date(benchmark):
    serializer = Serializer(date)
    benchmark.group = 'date'
    data = date.today()
    with check_refcount():
        benchmark(repeat(lambda: serializer.dump(data)))


def test_load_date(benchmark):
    serializer = Serializer(date)
    benchmark.group = 'date'
    data = date.today().isoformat()
    with check_refcount():
        benchmark(repeat(lambda: serializer.load(data)))


def test_dump_time(benchmark):
    serializer = Serializer(time)
    benchmark.group = 'time'
    data = datetime.now().time()
    with check_refcount():
        benchmark(repeat(lambda: serializer.dump(data)))


def test_load_time(benchmark):
    serializer = Serializer(time)
    benchmark.group = 'time'
    data = datetime.now().time().isoformat()
    with check_refcount():
        benchmark(repeat(lambda: serializer.load(data)))


def test_dump_datetime(benchmark):
    serializer = Serializer(datetime)
    benchmark.group = 'datetime'
    data = datetime.now()
    with check_refcount():
        benchmark(repeat(lambda: serializer.dump(data)))


def test_load_datetime(benchmark):
    serializer = Serializer(datetime)
    benchmark.group = 'datetime'
    data = datetime.now().isoformat()
    with check_refcount():
        benchmark(repeat(lambda: serializer.load(data)))


def test_dump_decimal(benchmark):
    serializer = Serializer(Decimal)
    benchmark.group = 'decimal'
    data = Decimal('1.3')
    with check_refcount():
        benchmark(repeat(lambda: serializer.dump(data)))


def test_load_decimal(benchmark):
    serializer = Serializer(Decimal)
    benchmark.group = 'decimal'
    data = '1.3'
    with check_refcount():
        benchmark(repeat(lambda: serializer.load(data)))


class FooEunm(enum.Enum):
    foo = 'foo'
    bar = 'bar'


def test_dump_enum(benchmark):
    serializer = Serializer(FooEunm)
    benchmark.group = 'enum'
    data = FooEunm.bar
    with check_refcount():
        benchmark(repeat(lambda: serializer.dump(data)))


def test_load_enum(benchmark):
    serializer = Serializer(FooEunm)
    benchmark.group = 'enum'
    data = 'foo'
    with check_refcount():
        benchmark(repeat(lambda: serializer.load(data)))


@dataclass
class FooDataclass:
    foo: int
    bar: str


def test_dump_dataclass(benchmark):
    serializer = Serializer(FooDataclass)
    benchmark.group = 'dataclass'
    data = FooDataclass(foo=1, bar='2')
    with check_refcount():
        benchmark(repeat(lambda: serializer.dump(data)))


def test_load_dataclass(benchmark):
    serializer = Serializer(FooDataclass)
    benchmark.group = 'dataclass'
    data = {'foo': 1, 'bar': '2'}
    with check_refcount():
        benchmark(repeat(lambda: serializer.load(data)))


@dataclass
class Node:
    value: str
    next: Optional['Node'] = None


@dataclass
class Root:
    head: Node


def test_dump_recursive(benchmark):
    serializer = Serializer(Root)
    benchmark.group = 'recursive'
    data = Root(
        head=Node(
            value='1',
            next=Node(value='2'),
        ),
    )
    with check_refcount():
        benchmark(repeat(lambda: serializer.dump(data)))


def test_load_recursive(benchmark):
    serializer = Serializer(Root)
    benchmark.group = 'recursive'
    data = {'head': {'next': {'next': None, 'value': '2'}, 'value': '1'}}
    with check_refcount():
        benchmark(repeat(lambda: serializer.load(data)))
