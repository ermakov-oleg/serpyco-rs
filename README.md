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
- Support deserialization from query string parameters (MultiDict like structures) with from string coercion

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
* Literal[str, int, Enum.variant, ...]
* Unions / Tagged unions
* typing.NewType
* PEP 695 (Type Parameter Syntax) - Python 3.12+

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
* Flatten


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

### Unions

`serpyco-rs` supports unions of types.

```python
from dataclasses import dataclass
from serpyco_rs import Serializer

@dataclass
class Foo:
    val: int

ser = Serializer(Foo | int)

print(ser.load({'val': 1}))
>> Foo(val=1)
print(ser.load(1))
>> 1
```

But performance of unions is worse than for single dataclasses. Because we need to check all possible types in the union.
For better performance, you can use [Tagged unions](#tagged-unions).


### Tagged unions

Supports tagged joins with discriminator field.

All classes in the union must be dataclasses or attrs with discriminator field `Literal[str]` or `Literal[Enum.variant]`.

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

#### Handling Python keywords with `python_field`

When the discriminator field name conflicts with Python keywords (like `type`, `from`, etc.),
you can use a trailing underscore in the Python field name and specify the JSON field name separately:

```python
from typing import Annotated, Literal
from dataclasses import dataclass
from serpyco_rs import Serializer
from serpyco_rs.metadata import Discriminator

@dataclass
class Foo:
    type_: Literal['foo']  # Python field name with underscore
    value: int

@dataclass
class Bar:
    type_: Literal['bar']  # Python field name with underscore
    value: str

# Specify JSON field name ('type') and Python field name ('type_')
ser = Serializer(list[Annotated[Foo | Bar, Discriminator('type', python_field='type_')]])

# JSON uses 'type' (without underscore)
print(ser.load([{'type': 'foo', 'value': 1}, {'type': 'bar', 'value': 'buz'}]))
>>> [Foo(type_='foo', value=1), Bar(type_='bar', value='buz')]
```

You can also use `python_field` to map different field names:

```python
@dataclass
class Foo:
    kind: Literal['foo']  # Python field name
    value: int

# JSON field 'type' maps to Python field 'kind'
ser = Serializer(Annotated[Foo, Discriminator('type', python_field='kind')])
print(ser.load({'type': 'foo', 'value': 1}))
>>> Foo(kind='foo', value=1)
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

### Flatten

`Flatten` allows you to flatten nested structures into the parent structure, similar to serde's `flatten` attribute in Rust.

```python
from dataclasses import dataclass
from typing import Annotated, Any
from serpyco_rs import Serializer
from serpyco_rs.metadata import Flatten

@dataclass
class Address:
    street: str
    city: str

@dataclass
class Person:
    name: str
    address: Annotated[Address, Flatten]        # Flatten struct fields
    extra: Annotated[dict[str, Any], Flatten]   # Collect additional properties

ser = Serializer(Person)

person = Person(
    name="John",
    address=Address(street="123 Main St", city="New York"),
    extra={"phone": "555-1234"}
)

# Serialization flattens all nested fields
result = ser.dump(person)
>> {'name': 'John', 'street': '123 Main St', 'city': 'New York', 'phone': '555-1234'}

# Deserialization reconstructs nested structures and collects extra fields
loaded = ser.load({'name': 'Jane', 'street': '456 Oak Ave', 'city': 'LA', 'email': 'jane@example.com'})
>> Person(name='Jane', address=Address(street='456 Oak Ave', city='LA'), extra={'email': 'jane@example.com'})
```

**Validation Rules:**
- Only one dict flatten field per dataclass/TypedDict
- No field name conflicts between regular and struct flatten fields (use `Alias` to resolve)
- Only dataclass, TypedDict, and dict types can be flattened

**JSON Schema:** Flattened struct fields appear as top-level properties; objects with dict flatten have `additionalProperties: true`


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


## PEP 695 Support

`serpyco-rs` supports the type parameter syntax from PEP 695, which was introduced in Python 3.12. This allows you to use a more concise and readable syntax for generic types.

### Generic Dataclasses

```python
from dataclasses import dataclass
from serpyco_rs import Serializer

@dataclass
class Container[T]:
    value: T
    items: list[T]

# Usage with concrete type
ser = Serializer(Container[int])

result = ser.dump(Container(value=42, items=[1, 2, 3]))
print(result)
>> {'value': 42, 'items': [1, 2, 3]}

loaded = ser.load({'value': 42, 'items': [1, 2, 3]})
print(loaded)
>> Container(value=42, items=[1, 2, 3])
```

### Type Aliases

```python
from dataclasses import dataclass
from serpyco_rs import Serializer

# New type alias syntax from PEP 695
type StrList = list[str]
type StrKeyDict[T] = dict[str, T]

@dataclass
class Data:
    names: StrList
    values: StrKeyDict[int]

ser = Serializer(Data)

result = ser.dump(Data(names=['alice', 'bob'], values={'a': 1, 'b': 2}))
print(result)
>> {'names': ['alice', 'bob'], 'values': {'a': 1, 'b': 2}}
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
    '$ref': '#/components/schemas/A',
    'components': {
        'schemas': {
            'A': {
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

Also, you can configure the schema generation via `JsonSchemaBuilder`.


```python
from dataclasses import dataclass
from serpyco_rs import Serializer, JsonSchemaBuilder

@dataclass
class A:
    foo: int
    bar: str

ser = Serializer(A)

builder = JsonSchemaBuilder(
  add_dialect_uri=False,
  ref_prefix='#/definitions',
)

print(builder.build(ser))
>> {'$ref': '#/definitions/__main__.A'}

print(builder.get_definitions())
>> {
  "__main__.A": {
    "properties": {
      "foo": {
        "type": "integer"
      },
      "bar": {
        "type": "string"
      }
    },
    "required": [
      "foo",
      "bar"
    ],
    "type": "object"
  }
}
```

## Query string deserialization

`serpyco-rs` can deserialize query string parameters (MultiDict like structures) with from string coercion.

```python

from dataclasses import dataclass
from urllib.parse import parse_qsl

from serpyco_rs import Serializer
from multidict import MultiDict

@dataclass
class A:
    foo: int
    bar: str

ser = Serializer(A)

print(ser.load_query_params(MultiDict(parse_qsl('foo=1&bar=2'))))
>> A(foo=1, bar='2')
```

## Custom Type Support

In `serpyco-rs`, you can add support for your own types by using the `custom_type_resolver` parameter and the `CustomType` class. This allows you to define how your custom types should be serialized and deserialized.

### CustomType

The `CustomType` class is a way to define how a custom type should be serialized and deserialized. It is a generic class that takes two type parameters: the type of the object to be serialized/deserialized and the type of the serialized/deserialized object.

Here is an example of a `CustomType` for `IPv4Address`:

```python
from serpyco_rs import CustomType
from ipaddress import IPv4Address, AddressValueError

class IPv4AddressType(CustomType[IPv4Address, str]):
    def serialize(self, obj: IPv4Address) -> str:
        return str(obj)

    def deserialize(self, data: str) -> IPv4Address:
        try:
            return IPv4Address(data)
        except AddressValueError:
            raise ValueError(f"Invalid IPv4 address: {data}")

    def get_json_schema(self) -> dict:
        return {"type": "string", "format": "ipv4"}
```

In this example, `IPv4AddressType` is a `CustomType` that serializes `IPv4Address` objects to strings and deserializes strings to `IPv4Address` objects. The `get_json_schema` method returns the JSON schema for the custom type.

### custom_type_resolver

The `custom_type_resolver` is a function that takes a type as input and returns an instance of `CustomType` if the type is supported, or `None` otherwise. This function is passed to the `Serializer` constructor.

Here is an example of a `custom_type_resolver` that supports `IPv4Address`:

```python
def custom_type_resolver(t: type) -> CustomType | None
    if t is IPv4Address:
        return IPv4AddressType()
    return None

ser = Serializer(MyDataclass, custom_type_resolver=custom_type_resolver)
```

In this example, the `custom_type_resolver` function checks if the type is `IPv4Address` and returns an instance of `IPv4AddressType` if it is. Otherwise, it returns `None`. This function is then passed to the `Serializer` constructor, which uses it to handle `IPv4Address` fields in the dataclass.

### Full Example

```python
from dataclasses import dataclass
from ipaddress import IPv4Address
from serpyco_rs import Serializer, CustomType

# Define custom type for IPv4Address
class IPv4AddressType(CustomType[IPv4Address, str]):
    def serialize(self, value: IPv4Address) -> str:
        return str(value)

    def deserialize(self, value: str) -> IPv4Address:
        return IPv4Address(value)

    def get_json_schema(self):
        return {
            'type': 'string',
            'format': 'ipv4',
        }

# Defining custom_type_resolver
def custom_type_resolver(t: type) -> CustomType | None:
    if t is IPv4Address:
        return IPv4AddressType()
    return None

@dataclass
class Data:
    ip: IPv4Address

# Use custom_type_resolver in Serializer
serializer = Serializer(Data, custom_type_resolver=custom_type_resolver)

# Example usage
data = Data(ip=IPv4Address('1.1.1.1'))
serialized_data = serializer.dump(data)  # {'ip': '1.1.1.1'}
deserialized_data = serializer.load(serialized_data)  # Data(ip=IPv4Address('1.1.1.1'))
```
