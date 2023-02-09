import enum
import uuid
from dataclasses import dataclass
from datetime import date, datetime, time
from decimal import Decimal
from typing import Optional

import pytest
from serpyco_rs import Serializer

from .utils import repeat


def test_dump_simple_types(benchmark):
    serializer = Serializer(int)
    benchmark(repeat(lambda: serializer.dump(1)))


def test_load_simple_types(benchmark):
    serializer = Serializer(int)
    benchmark(repeat(lambda: serializer.load(1)))


def test_dump_optional(benchmark):
    serializer = Serializer(Optional[int])

    @benchmark
    def inner():
        repeat(lambda: serializer.dump(1))
        repeat(lambda: serializer.dump(None))


def test_load_optional(benchmark):
    serializer = Serializer(Optional[int])

    @benchmark
    def inner():
        repeat(lambda: serializer.load(1))
        repeat(lambda: serializer.load(None))


def test_dump_list_simple_types(benchmark):
    serializer = Serializer(list[int])
    data = list(range(1000))
    benchmark(repeat(lambda: serializer.dump(data)))


def test_load_list_simple_types(benchmark):
    serializer = Serializer(list[int])
    data = list(range(1000))
    benchmark(repeat(lambda: serializer.load(data)))


def test_dump_tuple_simple_types(benchmark):
    serializer = Serializer(tuple[int, str, bool])
    benchmark(repeat(lambda: serializer.dump((123, "foo", True))))


def test_load_tuple_simple_types(benchmark):
    serializer = Serializer(tuple[int, str, bool])
    benchmark(repeat(lambda: serializer.load((123, "foo", True))))


def test_dump_dict_simple_types(benchmark):
    serializer = Serializer(dict[str, int])
    data = {str(i): i for i in range(1000)}
    benchmark(repeat(lambda: serializer.dump(data)))


@pytest.mark.slowtest
def test_load_dict_simple_types(benchmark):
    serializer = Serializer(dict[str, int])
    data = {str(i): i for i in range(1000)}
    benchmark(repeat(lambda: serializer.load(data), count=100))


def test_dump_uuid(benchmark):
    serializer = Serializer(uuid.UUID)
    data = uuid.uuid4()
    benchmark(repeat(lambda: serializer.dump(data)))


def test_load_uuid(benchmark):
    serializer = Serializer(uuid.UUID)
    data = str(uuid.uuid4())
    benchmark(repeat(lambda: serializer.load(data)))


def test_dump_date(benchmark):
    serializer = Serializer(date)
    data = date.today()
    benchmark(repeat(lambda: serializer.dump(data)))


def test_load_date(benchmark):
    serializer = Serializer(date)
    data = date.today().isoformat()
    benchmark(repeat(lambda: serializer.load(data)))


def test_dump_time(benchmark):
    serializer = Serializer(time)
    data = datetime.now().time()
    benchmark(repeat(lambda: serializer.dump(data)))


def test_load_time(benchmark):
    serializer = Serializer(time)
    data = datetime.now().time().isoformat()
    benchmark(repeat(lambda: serializer.load(data)))


def test_dump_datetime(benchmark):
    serializer = Serializer(datetime)
    data = datetime.now()
    benchmark(repeat(lambda: serializer.dump(data)))


def test_load_datetime(benchmark):
    serializer = Serializer(datetime)
    data = datetime.now().isoformat()
    benchmark(repeat(lambda: serializer.load(data)))


def test_dump_decimal(benchmark):
    serializer = Serializer(Decimal)
    data = Decimal("1.3")
    benchmark(repeat(lambda: serializer.dump(data)))


def test_load_decimal(benchmark):
    serializer = Serializer(Decimal)
    data = "1.3"
    benchmark(repeat(lambda: serializer.load(data)))


class FooEunm(enum.Enum):
    foo = "foo"
    bar = "bar"


def test_dump_enum(benchmark):
    serializer = Serializer(FooEunm)
    data = FooEunm.bar
    benchmark(repeat(lambda: serializer.dump(data)))


def test_load_enum(benchmark):
    serializer = Serializer(FooEunm)
    data = "foo"
    benchmark(repeat(lambda: serializer.load(data)))


@dataclass
class FooDataclass:
    foo: int
    bar: str


def test_dump_dataclass(benchmark):
    serializer = Serializer(FooDataclass)
    data = FooDataclass(foo=1, bar="2")
    benchmark(repeat(lambda: serializer.dump(data)))


def test_load_dataclass(benchmark):
    serializer = Serializer(FooDataclass)
    data = {"foo": 1, "bar": "2"}
    benchmark(repeat(lambda: serializer.load(data)))


@dataclass
class Node:
    value: str
    next: Optional["Node"] = None


@dataclass
class Root:
    head: Node


def test_dump_recursive(benchmark):
    serializer = Serializer(Root)
    data = Root(
        head=Node(
            value="1",
            next=Node(value="2"),
        ),
    )
    benchmark(repeat(lambda: serializer.dump(data)))


def test_load_recursive(benchmark):
    serializer = Serializer(Root)
    data = {"head": {"next": {"next": None, "value": "2"}, "value": "1"}}
    benchmark(repeat(lambda: serializer.load(data)))
