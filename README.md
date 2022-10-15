# serpyco-rs: a serializer for python dataclasses

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


## Features

- Serialization and unserialization of dataclasses
- Validation of input/output data
- Very fast

## Todo

- parse DateTime without timezone
- parse timezone for datetime.time
- omit_none
- run tests in CI
- CI checks (pylint, black, mypy, ...)
- more tests
- bench results
- write docs
- ...
