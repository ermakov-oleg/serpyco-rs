import json
import re
import uuid
from dataclasses import dataclass, field
from datetime import date, datetime, time, timezone
from decimal import Decimal
from enum import Enum
from ipaddress import IPv4Address
from typing import Annotated, Any, Generic, Literal, Optional, TypeVar, Union

import pytest
from typing_extensions import NotRequired, TypedDict

import serpyco_rs
from serpyco_rs import JSON, SchemaValidationError, Serializer, ValidationError
from serpyco_rs._custom_types import CustomType
from serpyco_rs.metadata import CustomEncoder, Discriminator, Flatten, Max, MaxLength, Min, MinLength


def test_decode_error_exported():
    assert issubclass(serpyco_rs.DecodeError, ValueError)


class Color(Enum):
    RED = 'red'
    GREEN = 'green'


@dataclass
class Inner:
    name: str
    score: float


@dataclass
class Everything:
    an_int: int
    a_str: str
    a_float: float
    a_bool: bool
    a_none: Optional[int]
    a_list: list[int]
    a_dict: dict[str, int]
    a_tuple: tuple[int, str]
    a_decimal: Decimal
    a_uuid: uuid.UUID
    a_date: date
    a_time: time
    a_datetime: datetime
    an_enum: Color
    an_any: Any
    nested: Inner
    with_default: int = 5
    items: list[Inner] = field(default_factory=list)


EVERYTHING = Everything(
    an_int=42,
    a_str='hello "world"\n',
    a_float=1.5,
    a_bool=True,
    a_none=None,
    a_list=[1, 2, 3],
    a_dict={'a': 1},
    a_tuple=(1, 'x'),
    a_decimal=Decimal('1.100'),
    a_uuid=uuid.UUID('12345678-1234-5678-1234-567812345678'),
    a_date=date(2026, 7, 23),
    a_time=time(12, 30, 0),
    a_datetime=datetime(2026, 7, 23, 12, 30, 0, tzinfo=timezone.utc),
    an_enum=Color.RED,
    an_any={'nested': [1, 'two', None, {'deep': True}]},
    nested=Inner(name='inner', score=0.5),
    items=[Inner(name='a', score=1.0), Inner(name='b', score=2.0)],
)


# --- Models reused by the parity sweeps below ---------------------------------

T = TypeVar('T')


@dataclass
class GenericBox(Generic[T]):
    value: Optional[T] = None
    path: Optional[str] = None


@dataclass
class Pair:
    q: str
    w: int


@dataclass
class Node:
    value: str
    next: Optional['Node'] = None


@dataclass
class Root:
    head: Node


class MovieTD(TypedDict):
    name: str
    year: int


@dataclass
class Outer:
    inner: Inner


# A custom-encoded scalar: CustomType needs a custom_type_resolver (a Serializer
# kwarg), which the parametrized signature cannot pass, so the sweep exercises the
# custom serialization path through CustomEncoder on a known base type instead.
# The real CustomType path is covered by test_custom_type_resolver_parity.
UpperStr = Annotated[str, CustomEncoder(serialize=str.upper, deserialize=str.lower)]


# Untagged / discriminated union members (module-level for parametrization).
@dataclass
class UP:
    name: str
    score: float


@dataclass
class UA:
    a: int


@dataclass
class UB:
    b: str


@dataclass
class UCat:
    kind: Literal['cat']
    meow: str


@dataclass
class UDog:
    kind: Literal['dog']
    bark: str


PetT = Annotated[Union[UCat, UDog], Discriminator('kind')]


def test_dump_codec_roundtrip():
    s = Serializer(Everything)
    data = s.dump(EVERYTHING, codec=JSON)
    assert isinstance(data, bytes)
    assert s.load(data, codec=JSON) == EVERYTHING


def test_dump_codec_matches_dict_path():
    s = Serializer(Everything)
    assert json.loads(s.dump(EVERYTHING, codec=JSON)) == s.dump(EVERYTHING)


def test_codec_in_constructor():
    s = Serializer(Everything, codec=JSON)
    data = s.dump(EVERYTHING)
    assert isinstance(data, bytes)
    assert s.load(data) == EVERYTHING


def test_load_codec_accepts_str_bytearray_memoryview():
    s = Serializer(Inner, codec=JSON)
    raw = s.dump(Inner(name='x', score=1.0))
    for variant in (raw, raw.decode(), bytearray(raw), memoryview(raw)):
        assert s.load(variant) == Inner(name='x', score=1.0)


def test_per_call_codec_overrides_instance():
    s = Serializer(Inner)
    assert isinstance(s.dump(Inner(name='x', score=1.0), codec=JSON), bytes)
    assert isinstance(s.dump(Inner(name='x', score=1.0)), dict)


def test_big_int_roundtrip():
    s = Serializer(int, codec=JSON)
    big = 2**100
    assert s.load(s.dump(big)) == big
    assert json.loads(s.dump(big)) == big


def test_malformed_json_raises_decode_error():
    s = Serializer(Inner, codec=JSON)
    for bad in (b'{', b'', b'{"name": "x", "score": }', b'[1,]'):
        with pytest.raises(serpyco_rs.DecodeError):
            s.load(bad)


def test_trailing_garbage_on_valid_doc_raises_decode_error():
    s = Serializer(Inner, codec=JSON)
    valid = s.dump(Inner(name='x', score=1.0))
    with pytest.raises(serpyco_rs.DecodeError):
        s.load(valid + b' trailing')


def test_schema_invalid_wellformed_raises_schema_error():
    s = Serializer(Inner, codec=JSON)
    with pytest.raises(SchemaValidationError):
        s.load(b'{"a": 1}')


def test_schema_error_has_same_instance_path():
    s = Serializer(Everything)
    good = s.dump(EVERYTHING)
    good['nested'] = {'name': 'x', 'score': 'not a float'}
    with pytest.raises(SchemaValidationError) as dict_err:
        s.load(good)
    with pytest.raises(SchemaValidationError) as codec_err:
        s.load(json.dumps(good), codec=JSON)
    assert [(e.message, e.instance_path) for e in codec_err.value.errors] == [
        (e.message, e.instance_path) for e in dict_err.value.errors
    ]


def test_bytes_field_json_dump_raises():
    s = Serializer(bytes, codec=JSON)
    with pytest.raises(ValidationError):
        s.dump(b'raw')


def test_nan_raises():
    s = Serializer(float, codec=JSON)
    with pytest.raises(ValidationError):
        s.dump(float('nan'))


def test_scalar_edges():
    s_int = Serializer(int, codec=JSON)
    assert s_int.load(b'-9223372036854775808') == -(2**63)

    with pytest.raises(SchemaValidationError):
        s_int.load(b'1.5')  # float is not a valid int -> schema error, NOT DecodeError

    s_float = Serializer(float, codec=JSON)
    # Integer JSON in a float field returns an int, exactly like the dict path
    # (equality 1 == 1.0 holds; the point is the int type matches the dict path).
    dict_result = Serializer(float).load(1)
    codec_result = s_float.load(b'1')
    assert codec_result == dict_result
    assert type(codec_result) is type(dict_result)
    assert s_float.load(b'1.5') == 1.5
    assert type(s_float.load(b'1.5')) is float

    s_str = Serializer(str, codec=JSON)
    assert s_str.load('"a\\nb\\u0000"'.encode()) == 'a\nb\x00'
    assert s_str.load(s_str.dump('cyrillic sh and \x1f')) == 'cyrillic sh and \x1f'

    s_dt = Serializer(datetime, codec=JSON)
    val = datetime(2026, 7, 23, 12, 0, 0, tzinfo=timezone.utc)
    assert s_dt.load(s_dt.dump(val)) == val

    s_dec = Serializer(Decimal, codec=JSON)
    assert s_dec.load(b'"1.100"') == Decimal('1.100')
    assert s_dec.load(b'1.1') == Decimal('1.1')  # precision from raw text, not repr(float)


def test_containers_deep():
    s = Serializer(list[dict[str, list[int]]], codec=JSON)
    val = [{'a': [1, 2]}, {'b': []}]
    assert s.load(s.dump(val)) == val

    s2 = Serializer(tuple[int, str, float], codec=JSON)
    assert s2.load(s2.dump((1, 'x', 0.5))) == (1, 'x', 0.5)

    s3 = Serializer(dict[str, int], codec=JSON)
    assert json.loads(s3.dump({'k': 1})) == {'k': 1}
    assert s3.load(b'{"k": 1}') == {'k': 1}


def test_array_element_error_path():
    s = Serializer(list[int], codec=JSON)
    with pytest.raises(SchemaValidationError) as e:
        s.load(b'[1, "bad", 3]')
    # element error must carry the index path, exactly like the dict path
    dict_s = Serializer(list[int])
    with pytest.raises(SchemaValidationError) as d:
        dict_s.load([1, 'bad', 3])
    assert [(x.message, x.instance_path) for x in e.value.errors] == [
        (x.message, x.instance_path) for x in d.value.errors
    ]


def test_recursion_depth_codec():
    s = Serializer(Any, codec=JSON)
    deep = b'[' * 2000 + b'1' + b']' * 2000
    with pytest.raises(RecursionError):
        s.load(deep)


def test_entity_unknown_keys_skipped():
    s = Serializer(Inner, codec=JSON)
    assert s.load(b'{"name": "x", "unknown": {"deep": [1]}, "score": 1.0}') == Inner(name='x', score=1.0)


def test_entity_missing_required_error_parity():
    s = Serializer(Inner)
    sc = Serializer(Inner, codec=JSON)
    bad = {'name': 'x'}  # missing score
    with pytest.raises(SchemaValidationError) as d:
        s.load(bad)
    with pytest.raises(SchemaValidationError) as c:
        sc.load(json.dumps(bad))
    assert [(e.message, e.instance_path) for e in c.value.errors] == [
        (e.message, e.instance_path) for e in d.value.errors
    ]


def test_entity_field_error_path_parity():
    s = Serializer(Everything)
    sc = Serializer(Everything, codec=JSON)
    bad = s.dump(EVERYTHING)
    bad['nested'] = {'name': 'x', 'score': 'notfloat'}
    with pytest.raises(SchemaValidationError) as d:
        s.load(bad)
    with pytest.raises(SchemaValidationError) as c:
        sc.load(json.dumps(bad))
    assert [(e.message, e.instance_path) for e in c.value.errors] == [
        (e.message, e.instance_path) for e in d.value.errors
    ]


def test_entity_defaults_applied():
    s = Serializer(Everything, codec=JSON)
    payload = Serializer(Everything).dump(EVERYTHING)
    del payload['with_default']  # has default=5
    del payload['items']  # has default_factory=list
    obj = s.load(json.dumps(payload))
    assert obj.with_default == 5
    assert obj.items == []


def test_camelcase_codec():
    @dataclass
    class TwoWords:
        long_name: str

    s = Serializer(TwoWords, camelcase_fields=True, codec=JSON)
    assert json.loads(s.dump(TwoWords(long_name='a'))) == {'longName': 'a'}
    assert s.load(b'{"longName": "a"}') == TwoWords(long_name='a')


def test_omit_none_codec():
    @dataclass
    class WithOpt:
        a: Optional[int] = None
        b: int = 1

    s = Serializer(WithOpt, omit_none=True, codec=JSON)
    assert json.loads(s.dump(WithOpt())) == {'b': 1}


def test_typeddict_codec_roundtrip():
    class Movie(TypedDict):
        name: str
        year: int

    s = Serializer(Movie, codec=JSON)
    val: Movie = {'name': 'Blade Runner', 'year': 1982}
    assert json.loads(s.dump(val)) == {'name': 'Blade Runner', 'year': 1982}
    assert s.load(json.dumps(val)) == val


def test_typeddict_codec_matches_dict_path():
    class Movie(TypedDict):
        name: str
        year: int

    s = Serializer(Movie)
    sc = Serializer(Movie, codec=JSON)
    val: Movie = {'name': 'Blade Runner', 'year': 1982}
    # dump parity
    assert json.loads(sc.dump(val)) == s.dump(val)
    # load parity (including unknown-key skipping)
    raw = b'{"name": "x", "year": 1, "unknown": [1, 2]}'
    assert sc.load(raw) == s.load(json.loads(raw))


def test_typeddict_partial_codec_parity():
    class Movie(TypedDict):
        name: str
        year: NotRequired[int]

    s = Serializer(Movie)
    sc = Serializer(Movie, codec=JSON)
    val: Movie = {'name': 'x'}  # optional 'year' omitted
    assert json.loads(sc.dump(val)) == s.dump(val)
    assert sc.load(json.dumps(val)) == s.load(dict(val))


def test_flatten_parity_codec():
    @dataclass
    class Address:
        street: str
        city: str
        country: str

    @dataclass
    class Person:
        name: str
        age: int
        address: Annotated[Address, Flatten]
        extra: Annotated[dict[str, Any], Flatten]

    person = Person(
        name='John',
        age=30,
        address=Address(street='123 Main', city='NYC', country='USA'),
        extra={'phone': '555-1234'},
    )
    s_dict = Serializer(Person)
    s_codec = Serializer(Person, codec=JSON)
    # dump parity: streaming falls back to the bridge for flatten entities
    assert json.loads(s_codec.dump(person)) == s_dict.dump(person)
    # load parity: native flatten streaming round-trips exactly like the bridge did
    assert s_codec.load(s_codec.dump(person)) == person
    assert s_codec.load(json.dumps(s_dict.dump(person))) == person


def test_flatten_struct_only_parity_codec():
    @dataclass
    class Address:
        street: str
        city: str

    @dataclass
    class Person:
        name: str
        address: Annotated[Address, Flatten]

    person = Person(name='John', address=Address(street='123 Main', city='NYC'))
    s = Serializer(Person)
    sc = Serializer(Person, codec=JSON)
    assert json.loads(sc.dump(person)) == s.dump(person)
    assert sc.load(sc.dump(person)) == person
    assert sc.load(json.dumps(s.dump(person))) == s.load(s.dump(person))


def test_flatten_dict_only_parity_codec():
    @dataclass
    class Person:
        name: str
        extra: Annotated[dict[str, Any], Flatten]

    person = Person(name='John', extra={'phone': '555-1234', 'age': 30})
    s = Serializer(Person)
    sc = Serializer(Person, codec=JSON)
    assert json.loads(sc.dump(person)) == s.dump(person)
    assert sc.load(sc.dump(person)) == person
    assert sc.load(json.dumps(s.dump(person))) == s.load(s.dump(person))


def test_flatten_nested_parity_codec():
    # A flatten field whose own type contains a flatten field: the outer stream
    # collects unknowns for `address`, then `Address.load` (dict path, unaffected
    # by streaming) recurses into its own `geo` flatten field.
    @dataclass
    class GeoInfo:
        lat: float
        lon: float

    @dataclass
    class Address:
        street: str
        geo: Annotated[GeoInfo, Flatten]

    @dataclass
    class Person:
        name: str
        address: Annotated[Address, Flatten]

    person = Person(name='John', address=Address(street='123 Main', geo=GeoInfo(lat=1.0, lon=2.0)))
    s = Serializer(Person)
    sc = Serializer(Person, codec=JSON)
    assert json.loads(sc.dump(person)) == s.dump(person)
    assert sc.load(sc.dump(person)) == person
    assert sc.load(json.dumps(s.dump(person))) == s.load(s.dump(person))


def test_flatten_missing_optional_default_parity_codec():
    @dataclass
    class Address:
        street: str
        city: str = 'Unknown'

    @dataclass
    class Person:
        name: str
        address: Annotated[Address, Flatten]

    s = Serializer(Person)
    sc = Serializer(Person, codec=JSON)
    # 'city' is entirely absent from the wire payload; Address.city default applies.
    raw = b'{"name": "John", "street": "123 Main"}'
    assert sc.load(raw) == s.load(json.loads(raw))
    assert sc.load(raw) == Person(name='John', address=Address(street='123 Main', city='Unknown'))


def test_flatten_extra_unknown_keys_parity_codec():
    @dataclass
    class Address:
        street: str
        city: str

    @dataclass
    class Person:
        name: str
        address: Annotated[Address, Flatten]
        extra: Annotated[dict[str, Any], Flatten]

    s = Serializer(Person)
    sc = Serializer(Person, codec=JSON)
    raw = b'{"name": "John", "street": "123 Main", "city": "NYC", "phone": "555-1234", "note": "vip"}'
    expected = Person(
        name='John',
        address=Address(street='123 Main', city='NYC'),
        extra={'phone': '555-1234', 'note': 'vip'},
    )
    assert sc.load(raw) == expected
    assert sc.load(raw) == s.load(json.loads(raw))


def test_flatten_struct_only_extra_key_dropped_parity_codec():
    # With no dict-flatten catch-all, a truly unrecognized key is silently
    # dropped (same as the dict path: Address.load never looks it up).
    @dataclass
    class Address:
        street: str
        city: str

    @dataclass
    class Person:
        name: str
        address: Annotated[Address, Flatten]

    s = Serializer(Person)
    sc = Serializer(Person, codec=JSON)
    raw = b'{"name": "John", "street": "123 Main", "city": "NYC", "unexpected": 123}'
    assert sc.load(raw) == s.load(json.loads(raw))
    assert sc.load(raw) == Person(name='John', address=Address(street='123 Main', city='NYC'))


def test_flatten_field_error_path_parity_codec():
    @dataclass
    class Address:
        street: str
        city: str

    @dataclass
    class Person:
        name: str
        address: Annotated[Address, Flatten]

    s = Serializer(Person)
    sc = Serializer(Person, codec=JSON)
    bad = {'name': 'John', 'street': '123 Main', 'city': 123}
    with pytest.raises(SchemaValidationError) as d:
        s.load(bad)
    with pytest.raises(SchemaValidationError) as c:
        sc.load(json.dumps(bad))
    assert [(e.message, e.instance_path) for e in c.value.errors] == [
        (e.message, e.instance_path) for e in d.value.errors
    ]


def test_flatten_missing_required_error_parity_codec():
    @dataclass
    class Address:
        street: str
        city: str

    @dataclass
    class Person:
        name: str
        address: Annotated[Address, Flatten]

    s = Serializer(Person)
    sc = Serializer(Person, codec=JSON)
    bad = {'name': 'John', 'street': '123 Main'}  # city missing entirely
    with pytest.raises(SchemaValidationError) as d:
        s.load(bad)
    with pytest.raises(SchemaValidationError) as c:
        sc.load(json.dumps(bad))
    assert [(e.message, e.instance_path) for e in c.value.errors] == [
        (e.message, e.instance_path) for e in d.value.errors
    ]


def test_flatten_dict_value_error_path_parity_codec():
    @dataclass
    class Person:
        name: str
        extra: Annotated[dict[str, int], Flatten]

    s = Serializer(Person)
    sc = Serializer(Person, codec=JSON)
    bad = {'name': 'John', 'age': 'not-an-int'}
    with pytest.raises(SchemaValidationError) as d:
        s.load(bad)
    with pytest.raises(SchemaValidationError) as c:
        sc.load(json.dumps(bad))
    assert [(e.message, e.instance_path) for e in c.value.errors] == [
        (e.message, e.instance_path) for e in d.value.errors
    ]


def test_flatten_wrong_type_error_parity_codec():
    @dataclass
    class Address:
        street: str

    @dataclass
    class Person:
        name: str
        address: Annotated[Address, Flatten]

    s = Serializer(Person)
    sc = Serializer(Person, codec=JSON)
    with pytest.raises(SchemaValidationError) as d:
        s.load([1, 2, 3])
    with pytest.raises(SchemaValidationError) as c:
        sc.load(b'[1,2,3]')
    # instance_path parity is the invariant that matters here; the message text
    # itself embeds the raw wire bytes on the codec path vs a Python repr() on
    # the dict path (`[1,2,3]` vs `[1, 2, 3]`) -- a pre-existing wrong_type_err
    # formatting quirk shared by every entity (flatten or not), out of scope
    # for this native-flatten-streaming change.
    assert [e.instance_path for e in c.value.errors] == [e.instance_path for e in d.value.errors]
    assert 'not of type "object"' in c.value.errors[0].message
    assert 'not of type "object"' in d.value.errors[0].message


def test_typeddict_flatten_struct_only_parity_codec():
    class Address(TypedDict):
        street: str
        city: str

    class Person(TypedDict):
        name: str
        address: Annotated[Address, Flatten]

    person: Person = {'name': 'John', 'address': {'street': '123 Main', 'city': 'NYC'}}
    s = Serializer(Person)
    sc = Serializer(Person, codec=JSON)
    assert json.loads(sc.dump(person)) == s.dump(person)
    assert sc.load(sc.dump(person)) == person
    assert sc.load(json.dumps(s.dump(person))) == s.load(s.dump(person))


def test_typeddict_flatten_dict_only_parity_codec():
    class Person(TypedDict):
        name: str
        extra: Annotated[dict[str, Any], Flatten]

    person: Person = {'name': 'John', 'extra': {'phone': '555-1234', 'age': 30}}
    s = Serializer(Person)
    sc = Serializer(Person, codec=JSON)
    assert json.loads(sc.dump(person)) == s.dump(person)
    assert sc.load(sc.dump(person)) == person
    assert sc.load(json.dumps(s.dump(person))) == s.load(s.dump(person))


def test_typeddict_flatten_struct_and_dict_parity_codec():
    class Address(TypedDict):
        street: str
        city: str
        country: str

    class Person(TypedDict):
        name: str
        age: int
        address: Annotated[Address, Flatten]
        extra: Annotated[dict[str, Any], Flatten]

    person: Person = {
        'name': 'John',
        'age': 30,
        'address': {'street': '123 Main', 'city': 'NYC', 'country': 'USA'},
        'extra': {'phone': '555-1234'},
    }
    s = Serializer(Person)
    sc = Serializer(Person, codec=JSON)
    assert json.loads(sc.dump(person)) == s.dump(person)
    assert sc.load(sc.dump(person)) == person
    assert sc.load(json.dumps(s.dump(person))) == s.load(s.dump(person))


def test_typeddict_flatten_missing_optional_default_parity_codec():
    class Address(TypedDict):
        street: str
        city: NotRequired[str]

    class Person(TypedDict):
        name: str
        address: Annotated[Address, Flatten]

    s = Serializer(Person)
    sc = Serializer(Person, codec=JSON)
    # 'city' is entirely absent from the wire payload; Address.city is
    # NotRequired, so the dict-path default (None) applies.
    raw = b'{"name": "John", "street": "123 Main"}'
    assert sc.load(raw) == s.load(json.loads(raw))
    assert sc.load(raw) == {'name': 'John', 'address': {'street': '123 Main', 'city': None}}


def test_union_codec():
    @dataclass
    class P:
        name: str
        score: float

    s = Serializer(int | str | P, codec=JSON)
    # primitive members: streaming materializes via parse_any and reuses the load loop
    assert s.load(b'5') == 5
    assert s.load(b'"x"') == 'x'
    # container member: raw span is captured and re-parsed by the matching variant
    assert s.load(b'{"name": "n", "score": 1.0}') == P(name='n', score=1.0)
    # discriminator-free container with extra keys still round-trips to the entity
    assert s.load(b'{"score": 2.5, "name": "z"}') == P(name='z', score=2.5)
    # round-trip of primitive members (dataclass dump inside an untagged union is a
    # pre-existing dict-path behavior returning the object unchanged, so it is not
    # serializable to JSON; only the streaming load path is in scope here)
    assert s.load(s.dump(5)) == 5
    assert s.load(s.dump('x')) == 'x'


def test_union_all_fail_parity():
    def norm(errs):
        # serpyco appends a disambiguation counter to the union's type name on the
        # second Serializer built for the same type (repr "int | str" vs "int | str1").
        # That counter is a global-naming artifact: the streaming and dict paths of one
        # Serializer share self.repr, so strip the trailing counter to make the
        # cross-serializer message comparison meaningful.
        return [(re.sub(r'\d*"$', '"', e.message), e.instance_path) for e in errs]

    s = Serializer(int | str)
    sc = Serializer(int | str, codec=JSON)
    bad = [1, 2, 3]  # neither int nor str -> drives the container all-fail branch
    with pytest.raises(serpyco_rs.SchemaValidationError) as d:
        s.load(bad)
    with pytest.raises(serpyco_rs.SchemaValidationError) as c:
        sc.load(json.dumps(bad))
    assert norm(c.value.errors) == norm(d.value.errors)


def test_discriminated_union_codec():
    @dataclass
    class Cat:
        kind: Literal['cat']
        meow: str

    @dataclass
    class Dog:
        kind: Literal['dog']
        bark: str

    Pet = Annotated[Union[Cat, Dog], Discriminator('kind')]
    s = Serializer(Pet, codec=JSON)
    # discriminator NOT first key -> requires scan-ahead over the captured span
    assert s.load(b'{"meow": "m", "kind": "cat"}') == Cat(kind='cat', meow='m')
    assert s.load(s.dump(Dog(kind='dog', bark='woof'))) == Dog(kind='dog', bark='woof')

    # unknown tag + missing discriminator: parity with the dict-path
    sd = Serializer(Pet)
    for bad in ({'kind': 'fish', 'meow': 'm'}, {'meow': 'm'}):
        with pytest.raises(serpyco_rs.SchemaValidationError) as d:
            sd.load(bad)
        with pytest.raises(serpyco_rs.SchemaValidationError) as c:
            s.load(json.dumps(bad))
        assert [(e.message, e.instance_path) for e in c.value.errors] == [
            (e.message, e.instance_path) for e in d.value.errors
        ]


def test_untagged_union_dump_entity():
    @dataclass
    class P:
        name: str
        score: float

    # entity is the SECOND member, after a scalar whose dump doesn't validate
    s = Serializer(int | P, codec=JSON)
    data = s.dump(P(name='n', score=1.5))
    assert json.loads(data) == {'name': 'n', 'score': 1.5}
    assert s.load(data) == P(name='n', score=1.5)
    # scalar member still dumps correctly
    assert json.loads(s.dump(7)) == 7
    assert s.load(s.dump(7)) == 7


def test_untagged_union_dump_roundtrip_both_orders():
    @dataclass
    class A:
        a: int

    @dataclass
    class B:
        b: str

    s = Serializer(A | B, codec=JSON)
    assert s.load(s.dump(A(a=1))) == A(a=1)
    assert s.load(s.dump(B(b='x'))) == B(b='x')


# --- Task 10: parity sweeps ---------------------------------------------------
#
# Prove the JSON codec path matches the dict path across the type system. Each
# case is dumped through both paths (comparing the decoded codec JSON against the
# dict-path dump) and round-tripped through the codec. KNOWN INTENTIONAL
# DIVERGENCES are deliberately excluded here (see ROUNDTRIP_ONLY_CASES for unions
# and the comments below): big int in a PLAIN int field, untagged-union dump, and
# dump-side type coercion.

PARITY_CASES: list[tuple[Any, Any]] = [
    # scalars
    (int, 42),
    (int, -(2**63)),  # i64 lower boundary
    (Any, 2**100),  # big int is fine inside Any (not a plain int field)
    (float, 1.5),
    (float, 3.14),  # ryu shortest repr must round-trip bit-identically
    (str, 'hello "world"\n'),
    (str, 'cyrillic ш and control \x1f'),
    (bool, True),
    (bool, False),
    (Optional[int], None),
    (Optional[int], 7),
    (Decimal, Decimal('1.100')),  # precision preserved via raw number text
    (uuid.UUID, uuid.UUID('12345678-1234-5678-1234-567812345678')),
    (date, date(2026, 7, 23)),
    (time, time(12, 30, 0)),
    (datetime, datetime(2026, 7, 23, 12, 30, 0, tzinfo=timezone.utc)),
    (Color, Color.RED),  # enum by value
    (Color, Color.GREEN),
    (Literal['foo', 'bar'], 'foo'),  # literal
    # containers
    (list[int], [1, 2, 3]),
    (dict[str, int], {'a': 1, 'b': 2}),
    (tuple[int, str], (1, 'x')),  # tuple -> list on the wire, tuple back
    (tuple[int, str, float], (1, 'x', 0.5)),
    (list[dict[str, list[int]]], [{'a': [1, 2]}, {'b': []}]),  # deeply nested
    (Optional[list[Optional[int]]], [1, None, 2]),  # optional chain
    (Optional[list[Optional[int]]], None),
    # dataclasses / generics / recursion / typeddict / custom encoder
    (Inner, Inner(name='inner', score=0.5)),
    (Everything, EVERYTHING),  # big nested dataclass
    (GenericBox[bool], GenericBox(value=True, path='some_path')),  # generic dataclass
    (GenericBox[Pair], GenericBox(value=Pair(q='q', w=1), path='p')),  # generic w/ nested dc
    (GenericBox[int], GenericBox(value=1)),
    (Node, Node(value='1', next=Node(value='2'))),  # recursive
    (Root, Root(head=Node(value='a', next=None))),
    (MovieTD, {'name': 'Blade Runner', 'year': 1982}),  # TypedDict
    (UpperStr, 'abc'),  # custom encoder through both paths
]


# Untagged-union dump is quirky by design (the dict path returns the object
# unchanged and can even raise for A|B), so unions are proven with round-trip
# parity only, never dump-equality against the dict path.
ROUNDTRIP_ONLY_CASES: list[tuple[Any, Any]] = [
    (int | str, 5),
    (int | str, 'hello'),
    (int | UP, UP(name='n', score=1.5)),  # entity as the second member
    (int | UP, 7),
    (UA | UB, UA(a=1)),
    (UA | UB, UB(b='x')),
    (PetT, UCat(kind='cat', meow='m')),  # discriminated union
    (PetT, UDog(kind='dog', bark='woof')),
]


@pytest.mark.parametrize(('typ', 'value'), PARITY_CASES)
def test_parity_dump(typ, value):
    s = Serializer(typ)
    sc = Serializer(typ, codec=JSON)
    # Compare the decoded JSON of the codec dump to the dict-path dump.
    assert json.loads(sc.dump(value)) == s.dump(value)


@pytest.mark.parametrize(('typ', 'value'), PARITY_CASES)
def test_parity_roundtrip(typ, value):
    sc = Serializer(typ, codec=JSON)
    assert sc.load(sc.dump(value)) == value


@pytest.mark.parametrize(('typ', 'value'), ROUNDTRIP_ONLY_CASES)
def test_parity_roundtrip_only(typ, value):
    sc = Serializer(typ, codec=JSON)
    assert sc.load(sc.dump(value)) == value


def test_custom_type_resolver_parity():
    # CustomType needs a custom_type_resolver (a Serializer kwarg), which the
    # parametrized PARITY_CASES signature cannot supply, so its dump/round-trip
    # parity is checked directly here.
    class IPv4Type(CustomType[IPv4Address, str]):
        def serialize(self, value: IPv4Address) -> str:
            return str(value)

        def deserialize(self, value: str) -> IPv4Address:
            return IPv4Address(value)

        def get_json_schema(self):
            return {'type': 'string', 'format': 'ipv4'}

    def resolver(t: type):
        return IPv4Type() if t is IPv4Address else None

    @dataclass
    class Data:
        ip: IPv4Address

    val = Data(ip=IPv4Address('1.1.1.1'))
    s = Serializer(Data, custom_type_resolver=resolver)
    sc = Serializer(Data, custom_type_resolver=resolver, codec=JSON)
    assert json.loads(sc.dump(val)) == s.dump(val)
    assert sc.load(sc.dump(val)) == val


# --- Task 10: error-parity sweep ----------------------------------------------
#
# Each case is a Python value that FAILS validation on the dict path; the codec
# path (fed the JSON-encoded value) must produce the identical (message,
# instance_path) list. Only SINGLE-invalid-field cases are used so wire-order vs
# field-order multi-error reporting never matters and exact equality holds.
# Union all-fail is excluded because it carries a global-naming counter artifact
# in the message that needs normalization (covered by test_union_all_fail_parity).
# Big-int-for-plain-int is excluded (a deliberate divergence, not a bug).

ERROR_PARITY_CASES: list[tuple[Any, Any]] = [
    (int, '1'),  # wrong scalar type
    (str, 1),
    (list[int], [2, 3, 'foo']),  # array element error with index path
    (dict[str, int], {'foo': 1, 'bar': '2'}),  # dict value error with key path
    (Inner, {'name': 'x'}),  # missing required field
    (Outer, {'inner': {'name': 'x', 'score': 'notfloat'}}),  # nested field error path
    (Annotated[int, Min(10), Max(100)], 1),  # below Min
    (Annotated[int, Min(10), Max(100)], 101),  # above Max
    (Annotated[str, MinLength(6), MaxLength(8)], 'hi'),  # below MinLength
    (Annotated[str, MinLength(6), MaxLength(8)], 'hello world'),  # above MaxLength
    (Color, 'blue'),  # enum invalid value
    (Literal['foo', 'bar'], 1),  # literal invalid value
    (tuple[int, str], [1, 2]),  # tuple element type
    (tuple[int, str], [1]),  # tuple too short
    (tuple[int, str], [1, 'x', 3]),  # tuple too long
]


@pytest.mark.parametrize(('typ', 'bad'), ERROR_PARITY_CASES)
def test_error_parity(typ, bad):
    s = Serializer(typ)
    sc = Serializer(typ, codec=JSON)
    with pytest.raises(SchemaValidationError) as dict_err:
        s.load(bad)
    with pytest.raises(SchemaValidationError) as codec_err:
        sc.load(json.dumps(bad))
    assert [(e.message, e.instance_path) for e in codec_err.value.errors] == [
        (e.message, e.instance_path) for e in dict_err.value.errors
    ]


# --- Task 10: malformed-input (DecodeError) corpus ----------------------------
#
# Genuine JSON syntax errors, loaded through a permissive `Any` type so ONLY the
# syntax (not the schema) can fail. Each must raise DecodeError with an int
# position.


@pytest.mark.parametrize(
    'bad',
    [
        b'',
        b'{',
        b'}',
        b'[1,2',
        b'"unterminated',
        b'{"a": 1,}',
        b'nul',
        b'1e',
        b'{"a"}',
        b'{,}',
        b'[,]',
    ],
)
def test_decode_error_corpus(bad):
    s = Serializer(Any, codec=JSON)  # permissive: only JSON syntax can fail
    with pytest.raises(serpyco_rs.DecodeError) as e:
        s.load(bad)
    assert isinstance(e.value.position, int)


def test_codec_literal_bool_roundtrip():
    # bool-valued Literal: the codec must be able to decode its own dump output.
    s = Serializer(Literal[True, False], codec=JSON)
    sd = Serializer(Literal[True, False])
    assert s.dump(True) == b'true'
    assert s.dump(False) == b'false'
    assert s.load(b'true') is True
    assert s.load(b'false') is False
    assert s.load(b'true') is sd.load(True)  # parity with dict path


def test_codec_float_dump_rejects_bool():
    # dict-path dump is lenient (orjson emits `true`); the codec dump validates
    # types and must not silently reinterpret a bool as the integer 1.
    s = Serializer(float, codec=JSON)
    with pytest.raises(serpyco_rs.SchemaValidationError):
        s.dump(True)
    # a plain int is still accepted for a float field
    assert s.dump(2) == b'2'
    assert json.loads(s.dump(1.5)) == 1.5


def test_codec_bytes_union_dump_skips_to_next_member():
    # `bytes` before a serializable member must not abort union probing.
    s = Serializer(Union[bytes, str], codec=JSON)
    assert s.dump('x') == b'"x"'
    assert s.load(s.dump('x')) == 'x'
    # a genuine bytes value still errors clearly (no serializable member)
    with pytest.raises(serpyco_rs.ValidationError):
        s.dump(b'x')


def test_codec_dict_omit_none_validates_key():
    # under omit_none, a skipped None value must not mask key validation.
    s = Serializer(dict[Color, Optional[int]], codec=JSON, omit_none=True)
    sd = Serializer(dict[Color, Optional[int]], omit_none=True)
    bad = {'not_a_color': None}
    with pytest.raises(serpyco_rs.SchemaValidationError):
        sd.dump(bad)  # dict path validates the key
    with pytest.raises(serpyco_rs.SchemaValidationError):
        s.dump(bad)  # codec must too (currently returns b'{}')
    # valid case still omits None values and dumps present ones
    assert s.dump({Color.RED: None}) == b'{}'
    assert json.loads(s.dump({Color.RED: 5})) == {'red': 5}


def test_union_kind_narrowing_reads_off_the_cursor():
    # A union whose members occupy distinct JSON kinds is resolved by the kind
    # alone: the sole viable member reads straight off the cursor (no skip pass,
    # no re-parse), and every member still round-trips.
    @dataclass
    class P:
        name: str
        score: float

    s = Serializer(Union[P, str], codec=JSON)
    assert s.load(b'{"name": "n", "score": 1.5}') == P(name='n', score=1.5)
    assert s.load(b'"plain"') == 'plain'

    # Ambiguous kinds (both members accept a number) keep the probing path.
    si = Serializer(Union[int, float], codec=JSON)
    assert si.load(b'7') == 7
    assert si.load(b'1.5') == 1.5


def test_union_kind_narrowing_error_is_the_member_error():
    # With one viable member, its error surfaces as-is: the path points at the
    # offending field instead of the union root. This is a deliberate divergence
    # from the dict path, which reports "nothing matched" at the root.
    @dataclass
    class P:
        foo: int

    sc = Serializer(Union[int, P], codec=JSON)
    with pytest.raises(serpyco_rs.SchemaValidationError) as c:
        sc.load(b'{"foo": "not-an-int"}')
    (err,) = c.value.errors
    assert err.instance_path == 'foo'
    assert 'is not of type "integer"' in err.message

    # No member accepts the kind -> the union's own error at the root.
    with pytest.raises(serpyco_rs.SchemaValidationError) as c:
        sc.load(b'[1]')
    (err,) = c.value.errors
    assert err.instance_path == ''
    assert 'is not of type' in err.message


def test_union_kind_narrowing_keeps_optional_and_nested_members():
    # Optional accepts null on top of whatever it wraps, so `None` stays viable
    # next to a same-kind member instead of being narrowed away.
    @dataclass
    class P:
        foo: int

    s = Serializer(Union[Optional[P], str], codec=JSON)
    assert s.load(b'null') is None
    assert s.load(b'{"foo": 1}') == P(foo=1)
    assert s.load(b'"s"') == 's'
