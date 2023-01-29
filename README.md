# serpyco-rs: a serializer for python dataclasses
[![PyPI version](https://img.shields.io/pypi/v/serpyco-rs.svg)](https://pypi.org/project/serpyco-rs) [![Python
versions](https://img.shields.io/pypi/pyversions/serpyco-rs.svg)](https://pypi.org/project/serpyco-rs) [![CI status](https://github.com/ermakov-oleg/serpyco-rs/actions/workflows/CI.yml/badge.svg)](https://github.com/ermakov-oleg/serpyco-rs/actions)

## What is serpyco-rs ?


Serpyco is a serialization library for [Python 3.9+ dataclasses](https://docs.python.org/3/library/dataclasses.html) that works just by defining your dataclasses:

```python
import dataclasses
import serpyco_rs

@dataclasses.dataclass
class Example:
    name: str
    num: int
    tags: list[str]


serializer = serpyco_rs.Serializer(Example)

result = serializer.dump(Example(name="foo", num=2, tags=["hello", "world"]))
print(result)

>> {'name': 'foo', 'num': 2, 'tags': ['hello', 'world']}
```

serpyco-rs works by analysing the dataclass fields and can recognize many types : `list`, `tuple`, `Optional`... 
You can also embed other dataclasses in a definition.

The main use-case for serpyco-rs is to serialize objects for an API, but it can be helpful whenever you need to transform objects to/from builtin Python types.

## Installation
Use pip to install:

```bash
$ pip install serpyco-rs
```


## Features

- Serialization and unserialization of dataclasses
- Validation of input/output data
- Very fast
- Support recursive schemas

## Supported field types
There is support for generic types from the standard typing module:

* Decimal
* UUID
* Time
* Date
* DateTime
* Enum
* List
* Dict
* Mapping
* Sequence
* Tuple (fixed size)


## Benchmark

#### dump

| Library     |   Median latency (milliseconds) |   Operations per second |   Relative (latency) |
|-------------|---------------------------------|-------------------------|----------------------|
| serpyco_rs  |                            0.05 |                 22193.4 |                 1    |
| serpyco     |                            0.05 |                 20643.6 |                 1.07 |
| pydantic    |                            2.64 |                   377.6 |                58.57 |
| marshmallow |                            1.04 |                   958.9 |                23.18 |

#### load with validate

| Library     |   Median latency (milliseconds) |   Operations per second |   Relative (latency) |
|-------------|---------------------------------|-------------------------|----------------------|
| serpyco_rs  |                            0.24 |                  4231.7 |                 1    |
| serpyco     |                            0.29 |                  3505.4 |                 1.21 |
| pydantic    |                            2.05 |                   487.5 |                 8.72 |
| marshmallow |                            4.63 |                   215.4 |                19.69 |

#### load

| Library     |   Median latency (milliseconds) |   Operations per second |   Relative (latency) |
|-------------|---------------------------------|-------------------------|----------------------|
| serpyco_rs  |                            0.07 |                 13766.2 |                 1    |
| serpyco     |                            0.08 |                 12263.8 |                 1.12 |
| pydantic    |                            2.04 |                   490   |                27.97 |
| marshmallow |                            4.59 |                   217.9 |                63.14 |
