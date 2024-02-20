import enum
import uuid
from dataclasses import dataclass
from datetime import date, datetime, time
from decimal import Decimal
from typing import Optional


from serpyco_rs import Serializer
from .utils import repeat


def test_dump_simple_types(bench_or_check_refcount):
    serializer = Serializer(float)
    bench_or_check_refcount.group = 'simple_types'
    bench_or_check_refcount(repeat(lambda: serializer.dump(1)))


def test_load_simple_types(bench_or_check_refcount):
    serializer = Serializer(int)
    bench_or_check_refcount.group = 'simple_types'
    bench_or_check_refcount(repeat(lambda: serializer.load(1)))


def test_dump_optional(bench_or_check_refcount):
    serializer = Serializer(Optional[int])

    bench_or_check_refcount.group = 'optional'

    def inner():
        repeat(lambda: serializer.dump(1))
        repeat(lambda: serializer.dump(None))

    bench_or_check_refcount(inner)


def test_load_optional(bench_or_check_refcount):
    serializer = Serializer(Optional[int])

    bench_or_check_refcount.group = 'optional'

    def inner():
        repeat(lambda: serializer.load(1))
        repeat(lambda: serializer.load(None))

    bench_or_check_refcount(inner)


def test_dump_list_simple_types(bench_or_check_refcount):
    serializer = Serializer(list[int])
    bench_or_check_refcount.group = 'list'
    data = list(range(1000))
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


def test_load_list_simple_types(bench_or_check_refcount):
    serializer = Serializer(list[int])
    bench_or_check_refcount.group = 'list'
    data = list(range(1000))
    bench_or_check_refcount(repeat(lambda: serializer.load(data)))


def test_dump_tuple_simple_types(bench_or_check_refcount):
    serializer = Serializer(tuple[int, str, bool])
    bench_or_check_refcount.group = 'tuple'
    bench_or_check_refcount(repeat(lambda: serializer.dump((123, 'foo', True))))


def test_load_tuple_simple_types(bench_or_check_refcount):
    serializer = Serializer(tuple[int, str, bool])
    bench_or_check_refcount.group = 'tuple'
    bench_or_check_refcount(repeat(lambda: serializer.load((123, 'foo', True))))


def test_dump_dict_simple_types(bench_or_check_refcount):
    serializer = Serializer(dict[str, int])
    bench_or_check_refcount.group = 'dict'
    data = {str(i): i for i in range(1000)}
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


def test_dump_dict_dataclass_value(bench_or_check_refcount):
    @dataclass
    class Foo:
        foo: int

    serializer = Serializer(dict[str, Foo])
    bench_or_check_refcount.group = 'dict'
    data = {str(i): Foo(i) for i in range(12)}
    bench_or_check_refcount(repeat(lambda: serializer.dump(data), count=100))


def test_load_dict_simple_types(bench_or_check_refcount):
    serializer = Serializer(dict[str, int])
    bench_or_check_refcount.group = 'dict'
    data = {str(i): i for i in range(1000)}
    bench_or_check_refcount(repeat(lambda: serializer.load(data)))


def test_dump_uuid(bench_or_check_refcount):
    serializer = Serializer(uuid.UUID)
    bench_or_check_refcount.group = 'uuid'
    data = uuid.uuid4()
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


def test_load_uuid(bench_or_check_refcount):
    serializer = Serializer(uuid.UUID)
    bench_or_check_refcount.group = 'uuid'
    data = str(uuid.uuid4())
    bench_or_check_refcount(repeat(lambda: serializer.load(data)))


def test_dump_date(bench_or_check_refcount):
    serializer = Serializer(date)
    bench_or_check_refcount.group = 'date'
    data = date.today()
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


def test_load_date(bench_or_check_refcount):
    serializer = Serializer(date)
    bench_or_check_refcount.group = 'date'
    data = date.today().isoformat()
    bench_or_check_refcount(repeat(lambda: serializer.load(data)))


def test_dump_time(bench_or_check_refcount):
    serializer = Serializer(time)
    bench_or_check_refcount.group = 'time'
    data = datetime.now().time()
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


def test_load_time(bench_or_check_refcount):
    serializer = Serializer(time)
    bench_or_check_refcount.group = 'time'
    data = datetime.now().time().isoformat()
    bench_or_check_refcount(repeat(lambda: serializer.load(data)))


def test_dump_datetime(bench_or_check_refcount):
    serializer = Serializer(datetime)
    bench_or_check_refcount.group = 'datetime'
    data = datetime.now()
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


def test_load_datetime(bench_or_check_refcount):
    serializer = Serializer(datetime)
    bench_or_check_refcount.group = 'datetime'
    data = datetime.now().isoformat()
    bench_or_check_refcount(repeat(lambda: serializer.load(data)))


def test_dump_decimal(bench_or_check_refcount):
    serializer = Serializer(Decimal)
    bench_or_check_refcount.group = 'decimal'
    data = Decimal('1.3')
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


def test_load_decimal(bench_or_check_refcount):
    serializer = Serializer(Decimal)
    bench_or_check_refcount.group = 'decimal'
    data = '1.3'
    bench_or_check_refcount(repeat(lambda: serializer.load(data)))


class FooEunm(enum.Enum):
    foo = 'foo'
    bar = 'bar'


def test_dump_enum(bench_or_check_refcount):
    serializer = Serializer(FooEunm)
    bench_or_check_refcount.group = 'enum'
    data = FooEunm.bar
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


def test_load_enum(bench_or_check_refcount):
    serializer = Serializer(FooEunm)
    bench_or_check_refcount.group = 'enum'
    data = 'foo'
    bench_or_check_refcount(repeat(lambda: serializer.load(data)))


@dataclass
class FooDataclass:
    foo: int
    bar: str


def test_dump_dataclass(bench_or_check_refcount):
    serializer = Serializer(FooDataclass)
    bench_or_check_refcount.group = 'dataclass'
    data = FooDataclass(foo=1, bar='2')
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


def test_load_dataclass(bench_or_check_refcount):
    serializer = Serializer(FooDataclass)
    bench_or_check_refcount.group = 'dataclass'
    data = {'foo': 1, 'bar': '2'}
    bench_or_check_refcount(repeat(lambda: serializer.load(data)))


@dataclass
class Node:
    value: str
    next: Optional['Node'] = None


@dataclass
class Root:
    head: Node


def test_dump_recursive(bench_or_check_refcount):
    serializer = Serializer(Root)
    bench_or_check_refcount.group = 'recursive'
    data = Root(
        head=Node(
            value='1',
            next=Node(value='2'),
        ),
    )
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


def test_load_recursive(bench_or_check_refcount):
    serializer = Serializer(Root)
    bench_or_check_refcount.group = 'recursive'
    data = {'head': {'next': {'next': None, 'value': '2'}, 'value': '1'}}
    bench_or_check_refcount(repeat(lambda: serializer.load(data)))
