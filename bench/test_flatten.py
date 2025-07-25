from dataclasses import dataclass
from typing import Annotated, Any, TypedDict

from serpyco_rs import Serializer
from serpyco_rs.metadata import Flatten

from .utils import repeat


@dataclass
class Address:
    street: str
    city: str
    country: str


@dataclass
class PersonWithFlattenStruct:
    name: str
    age: int
    address: Annotated[Address, Flatten]


@dataclass
class PersonWithFlattenDict:
    name: str
    age: int
    metadata: Annotated[dict[str, Any], Flatten]


def test_dump_flatten_struct(bench_or_check_refcount):
    data = PersonWithFlattenStruct(
        name='John Doe', age=30, address=Address(street='123 Main St', city='New York', country='USA')
    )

    serializer = Serializer(PersonWithFlattenStruct)
    serializer.dump(data)  # warmup

    bench_or_check_refcount.group = 'flatten'
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


def test_load_flatten_struct(bench_or_check_refcount):
    data = {'name': 'John Doe', 'age': 30, 'street': '123 Main St', 'city': 'New York', 'country': 'USA'}

    serializer = Serializer(PersonWithFlattenStruct)
    serializer.load(data)  # warmup

    bench_or_check_refcount.group = 'flatten'
    bench_or_check_refcount(repeat(lambda: serializer.load(data)))


def test_dump_flatten_dict(bench_or_check_refcount):
    data = PersonWithFlattenDict(
        name='Jane Smith',
        age=25,
        metadata={'department': 'Engineering', 'level': 'Senior', 'projects': ['Project A', 'Project B']},
    )

    serializer = Serializer(PersonWithFlattenDict)
    serializer.dump(data)  # warmup

    bench_or_check_refcount.group = 'flatten'
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


def test_load_flatten_dict(bench_or_check_refcount):
    data = {
        'name': 'Jane Smith',
        'age': 25,
        'department': 'Engineering',
        'level': 'Senior',
        'projects': ['Project A', 'Project B'],
    }

    serializer = Serializer(PersonWithFlattenDict)
    serializer.load(data)  # warmup

    bench_or_check_refcount.group = 'flatten'
    bench_or_check_refcount(repeat(lambda: serializer.load(data)))


class AddressTypedDict(TypedDict):
    street: str
    city: str
    country: str


class PersonWithFlattenStructTypedDict(TypedDict):
    name: str
    age: int
    address: Annotated[AddressTypedDict, Flatten]


class PersonWithFlattenDictTypedDict(TypedDict):
    name: str
    age: int
    metadata: Annotated[dict[str, Any], Flatten]


def test_dump_flatten_struct_typeddict(bench_or_check_refcount):
    data = PersonWithFlattenStructTypedDict(
        name='John Doe', age=30, address=AddressTypedDict(street='123 Main St', city='New York', country='USA')
    )

    serializer = Serializer(PersonWithFlattenStructTypedDict)
    serializer.dump(data)  # warmup

    bench_or_check_refcount.group = 'flatten typeddict'
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


def test_load_flatten_struct_typeddict(bench_or_check_refcount):
    data = {'name': 'John Doe', 'age': 30, 'street': '123 Main St', 'city': 'New York', 'country': 'USA'}

    serializer = Serializer(PersonWithFlattenStructTypedDict)
    serializer.load(data)  # warmup

    bench_or_check_refcount.group = 'flatten typeddict'
    bench_or_check_refcount(repeat(lambda: serializer.load(data)))


def test_dump_flatten_dict_typeddict(bench_or_check_refcount):
    data = PersonWithFlattenDictTypedDict(
        name='Jane Smith',
        age=25,
        metadata={'department': 'Engineering', 'level': 'Senior', 'projects': ['Project A', 'Project B']},
    )

    serializer = Serializer(PersonWithFlattenDictTypedDict)
    serializer.dump(data)  # warmup

    bench_or_check_refcount.group = 'flatten typeddict'
    bench_or_check_refcount(repeat(lambda: serializer.dump(data)))


def test_load_flatten_dict_typeddict(bench_or_check_refcount):
    data = {
        'name': 'Jane Smith',
        'age': 25,
        'department': 'Engineering',
        'level': 'Senior',
        'projects': ['Project A', 'Project B'],
    }

    serializer = Serializer(PersonWithFlattenDictTypedDict)
    serializer.load(data)  # warmup

    bench_or_check_refcount.group = 'flatten typeddict'
    bench_or_check_refcount(repeat(lambda: serializer.load(data)))
