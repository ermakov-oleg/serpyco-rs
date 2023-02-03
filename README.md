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

- Serialization and deserialization of dataclasses
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

macOS Monterey / Apple M1 Pro / 16GB RAM / Python 3.11.0

#### dump

| Library     |   Median latency (milliseconds) |   Operations per second |   Relative (latency) |
|-------------|---------------------------------|-------------------------|----------------------|
| serpyco_rs  |                            0.05 |                 22188.2 |                 1    |
| serpyco     |                            0.05 |                 20878.5 |                 1.06 |
| mashumaro   |                            0.06 |                 15602.7 |                 1.42 |
| pydantic    |                            2.66 |                   375.6 |                59    |
| marshmallow |                            1.05 |                   951.7 |                23.33 |


#### load with validate

| Library     |   Median latency (milliseconds) |   Operations per second |   Relative (latency) |
|-------------|---------------------------------|-------------------------|----------------------|
| serpyco_rs  |                            0.23 |                  4400.1 |                 1    |
| serpyco     |                            0.28 |                  3546.4 |                 1.24 |
| mashumaro   |                            0.23 |                  4377.7 |                 1.01 |
| pydantic    |                            2.01 |                   497.3 |                 8.86 |
| marshmallow |                            4.55 |                   219.9 |                20.03 |


#### load (only serpyco and serpyco_rs supported load without validate)

| Library     |   Median latency (milliseconds) |   Operations per second |   Relative (latency) |
|-------------|---------------------------------|-------------------------|----------------------|
| serpyco_rs  |                            0.07 |                 13882.9 |                 1    |
| serpyco     |                            0.08 |                 12424.5 |                 1.12 |
| mashumaro   |                            0.23 |                  4382.9 |                 3.17 |
| pydantic    |                            2.02 |                   494.4 |                28.09 |
| marshmallow |                            4.59 |                   217.5 |                63.8  |


## Supported annotations

`serpyco-rs` supports changing load/dump behavior with `typing.Annotated`.

Currently available:
* Alias
* FiledFormat (CamelCase / NoFormat)
* NoneFormat (OmitNone / KeepNone)
* Min / Max
* MinLength / MaxLength


### Alias
`Alias` is needed to override the field name in the structure used for `load` / `dump`.

```python
from dataclasses import dataclass
from typing import Annotated
from serpyco_rs import Serializer
from serpyco_rs.metadata import Alias

@dataclass
class A:
    foo: Annotated[int, Alias('bar')]

ser = Serializer(A)

print(ser.load({'bar': 1}))
>> A(foo=1)

print(ser.dump(A(foo=1)))
>> {'bar': 1}
```

### FiledFormat
Used to have response bodies in camelCase while keeping your python code in snake_case.

```python
from dataclasses import dataclass
from typing import Annotated
from serpyco_rs import Serializer
from serpyco_rs.metadata import CamelCase, NoFormat

@dataclass
class B:
    buz_filed: str

@dataclass
class A:
    foo_filed: int
    bar_filed: Annotated[B, NoFormat]

ser = Serializer(Annotated[A, CamelCase])  # or ser = Serializer(A, camelcase_fields=True)

print(ser.dump(A(foo_filed=1, bar_filed=B(buz_filed='123'))))
>> {'fooFiled': 1, 'barFiled': {'buz_filed': '123'}}

print(ser.load({'fooFiled': 1, 'barFiled': {'buz_filed': '123'}}))
>> A(foo_filed=1, bar_filed=B(buz_filed='123'))
```

### NoneFormat
Via `OmitNone` we can drop None values for non required fields in the serialized dicts

```python
from dataclasses import dataclass
from serpyco_rs import Serializer

@dataclass
class A:
    required_val: bool | None
    optional_val: bool | None = None

serializer = Serializer(A, omit_none=True) # or Serializer(Annotated[A, OmitNone])

print(serializer.dump(A(required_val=None, optional_val=None)))
>>> {'required_val': None}
```

### Min / Max

Supported for `int` / `float` / `Decimal` types and only for validation on load.

```python
from typing import Annotated
from serpyco_rs import Serializer
from serpyco_rs.metadata import Min, Max

ser = Serializer(Annotated[int, Min(1), Max(10)])

ser.load(123)
>> SchemaValidationError: [ErrorItem(message='123 is greater than the maximum of 10', instance_path='', schema_path='maximum')]
```

### MinLength / MaxLength
`MinLength` / `MaxLength` can be used to restrict the length of loaded strings.

```python
from typing import Annotated
from serpyco_rs import Serializer
from serpyco_rs.metadata import MinLength

ser = Serializer(Annotated[str, MinLength(5)])

ser.load("1234")
>> SchemaValidationError: [ErrorItem(message='"1234" is shorter than 5 characters', instance_path='', schema_path='minLength')]
```
