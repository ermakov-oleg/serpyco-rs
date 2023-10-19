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

Inspired by [serpyco](https://pypi.org/project/serpyco/).

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
- Validation of input data
- Very fast
- Support recursive schemas
- Generate JSON Schema Specification (Draft 2020-12)
- Support custom encoders/decoders for fields

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
* Bytes (pass through)
* TypedDict
* Mapping
* Sequence
* Tuple (fixed size)
* Literal[str, ...]
* Tagged unions (restricted)

## Benchmarks

<details>
  <summary>Linux</summary>
  
  #### Load
  
  | Library     |   Median latency (milliseconds) |   Operations per second |   Relative (latency) |
  |-------------|---------------------------------|-------------------------|----------------------|
  | serpyco_rs  |                            0.16 |                  6318.1 |                 1    |
  | mashumaro   |                            0.45 |                  2244.4 |                 2.81 |
  | pydantic    |                            0.57 |                  1753.9 |                 3.56 |
  | serpyco     |                            0.82 |                  1228.3 |                 5.17 |
  | marshmallow |                            8.49 |                   117.4 |                53.35 |
  
  #### Dump
  
  | Library     |   Median latency (milliseconds) |   Operations per second |   Relative (latency) |
  |-------------|---------------------------------|-------------------------|----------------------|
  | serpyco_rs  |                            0.07 |                 13798   |                 1    |
  | serpyco     |                            0.07 |                 13622   |                 1.02 |
  | mashumaro   |                            0.1  |                 10219.5 |                 1.36 |
  | pydantic    |                            0.22 |                  4615.5 |                 2.99 |
  | marshmallow |                            2    |                   497   |                27.69 |
</details>


<details>
  <summary>MacOS</summary>
  macOS Monterey / Apple M1 Pro / 16GB RAM / Python 3.11.0
  
  #### Load
  
  | Library     |   Median latency (milliseconds) |   Operations per second |   Relative (latency) |
  |-------------|---------------------------------|-------------------------|----------------------|
  | serpyco_rs  |                            0.1  |                  9865.1 |                 1    |
  | mashumaro   |                            0.2  |                  4968   |                 2    |
  | pydantic    |                            0.34 |                  2866.7 |                 3.42 |
  | serpyco     |                            0.69 |                  1444.1 |                 6.87 |
  | marshmallow |                            4.14 |                   241.8 |                41.05 |

  #### Dump
  
  | Library     |   Median latency (milliseconds) |   Operations per second |   Relative (latency) |
  |-------------|---------------------------------|-------------------------|----------------------|
  | serpyco_rs  |                            0.04 |                 22602.6 |                 1    |
  | serpyco     |                            0.05 |                 21232.9 |                 1.06 |
  | mashumaro   |                            0.06 |                 15903.4 |                 1.42 |
  | pydantic    |                            0.16 |                  6262.6 |                 3.61 |
  | marshmallow |                            1.04 |                   962   |                23.5  |
</details>


## Supported annotations

`serpyco-rs` supports changing load/dump behavior with `typing.Annotated`.

Currently available:
* Alias
* FieldFormat (CamelCase / NoFormat)
* NoneFormat (OmitNone / KeepNone)
* Discriminator
* Min / Max
* MinLength / MaxLength
* CustomEncoder
* NoneAsDefaultForOptional (ForceDefaultForOptional)


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

### FieldFormat
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

ser = Serializer(A, omit_none=True) # or Serializer(Annotated[A, OmitNone])

print(ser.dump(A(required_val=None, optional_val=None)))
>>> {'required_val': None}
```

### Tagged unions

Supports tagged joins with discriminator field.

All classes in the union must be dataclasses or attrs with discriminator field `Literal[str]`.

**The discriminator field is always mandatory.**

```python
from typing import Annotated, Literal
from dataclasses import dataclass
from serpyco_rs import Serializer
from serpyco_rs.metadata import Discriminator

@dataclass
class Foo:
    type: Literal['foo']
    value: int

@dataclass(kw_only=True)
class Bar:
    type: Literal['bar'] = 'bar'
    value: str

ser = Serializer(list[Annotated[Foo | Bar, Discriminator('type')]])

print(ser.load([{'type': 'foo', 'value': 1}, {'type': 'bar', 'value': 'buz'}]))
>>> [Foo(type='foo', value=1), Bar(type='bar', value='buz')]
```

### Min / Max

Supported for `int` / `float` / `Decimal` types and only for validation on load.

```python
from typing import Annotated
from serpyco_rs import Serializer
from serpyco_rs.metadata import Min, Max

ser = Serializer(Annotated[int, Min(1), Max(10)])

ser.load(123)
>> SchemaValidationError: [ErrorItem(message='123 is greater than the maximum of 10', instance_path='')]
```

### MinLength / MaxLength
`MinLength` / `MaxLength` can be used to restrict the length of loaded strings.

```python
from typing import Annotated
from serpyco_rs import Serializer
from serpyco_rs.metadata import MinLength

ser = Serializer(Annotated[str, MinLength(5)])

ser.load("1234")
>> SchemaValidationError: [ErrorItem(message='"1234" is shorter than 5 characters', instance_path='')]
```

### NoneAsDefaultForOptional
`ForceDefaultForOptional` / `KeepDefaultForOptional` can be used to set None as default value for optional (nullable) fields.

```python
from dataclasses import dataclass
from serpyco_rs import Serializer


@dataclass
class Foo:
    val: int                 # not nullable + required
    val1: int | None         # nullable + required
    val2: int | None = None  # nullable + not required

ser_force_default = Serializer(Foo, force_default_for_optional=True)  # or Serializer(Annotated[Foo, ForceDefaultForOptional])
ser = Serializer(Foo)

# all fields except val are optional and nullable
assert ser_force_default.load({'val': 1}) == Foo(val=1, val1=None, val2=None) 

# val1 field is required and nullable and val1 should be present in the dict
ser.load({'val': 1})
>> SchemaValidationError: [ErrorItem(message='"val1" is a required property', instance_path='')]
```


### Custom encoders for fields

You can provide CustomEncoder with `serialize` and `deserialize` functions, or `serialize_with` and `deserialize_with` annotations.

```python
from typing import Annotated
from dataclasses import dataclass
from serpyco_rs import Serializer
from serpyco_rs.metadata import CustomEncoder

@dataclass
class Foo:
    val: Annotated[str, CustomEncoder[str, str](serialize=str.upper, deserialize=str.lower)]

ser = Serializer(Foo)
val = ser.dump(Foo(val='bar'))
>> {'val': 'BAR'}
assert ser.load(val) == Foo(val='bar') 
```

**Note:** `CustomEncoder` has no effect to validation and JSON Schema generation.

### Bytes fields

`serpyco-rs` can loads bytes fields as is (without base64 encoding and validation).

```python
from dataclasses import dataclass
from serpyco_rs import Serializer

@dataclass
class Foo:
    val: bytes

ser = Serializer(Foo)
ser.load({'val': b'123'}) == Foo(val=b'123')
```


## Getting JSON Schema

`serpyco-rs` can generate JSON Schema for your dataclasses (Draft 2020-12).

```python
from dataclasses import dataclass
from serpyco_rs import Serializer

@dataclass
class A:
    """Description of A"""
    foo: int
    bar: str

ser = Serializer(A)

print(ser.get_json_schema())
>> {
    '$schema': 'https://json-schema.org/draft/2020-12/schema', 
    '$ref': '#/components/schemas/A[no_format,keep_nones]', 
    'components': {
        'schemas': {
            'A[no_format,keep_nones]': {
                'properties': {
                    'foo': {'type': 'integer'}, 
                    'bar': {'type': 'string'}
                }, 
                'required': ['foo', 'bar'], 
                'type': 'object', 
                'description': 'Description of A'
            }
        }
    }
}
```
