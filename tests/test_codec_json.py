import json
import re
import uuid
from dataclasses import dataclass, field
from datetime import date, datetime, time, timezone
from decimal import Decimal
from enum import Enum, IntEnum
from ipaddress import IPv4Address
from typing import Annotated, Any, Generic, Literal, Optional, TypeVar, Union

import pytest
from typing_extensions import NotRequired, TypedDict

import serpyco_rs
from serpyco_rs import JSON, Codec, SchemaValidationError, Serializer, ValidationError
from serpyco_rs._custom_types import CustomType
from serpyco_rs.metadata import CustomEncoder, Discriminator, Flatten, Max, MaxLength, Min, MinLength


# One codec per supported byte format. Adding msgpack later is exactly one entry
# here — every codec-agnostic test below is already parametrized over this list,
# and each id is derived from the codec itself. Mirrors bench/test_codec_encoders.py
# (`CODECS` / `_codec_id` / `parametrize_codec`) on purpose: keeping the two files
# consistent matters more than inventing a nicer scheme here.
CODECS = [JSON]


def _codec_id(codec: Codec) -> str:
    return codec._name


parametrize_codec = pytest.mark.parametrize('codec', CODECS, ids=_codec_id)


def _dump_any(codec: Codec, value: Any) -> bytes:
    """Encode an arbitrary Python value to `codec`'s wire bytes through the generic
    Any bridge. Stands in for a format-specific literal (`json.dumps`) so a
    codec-agnostic test can build `load()` input without assuming JSON's syntax."""
    return Serializer(Any, codec=codec).dump(value)


def _load_any(codec: Codec, raw: bytes) -> Any:
    """Decode `codec` wire bytes back to a plain Python structure through the generic
    Any bridge. Stands in for `json.loads` so a codec-agnostic test can inspect a
    dump's shape without assuming JSON's syntax."""
    return Serializer(Any, codec=codec).load(raw)


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


# CustomType needs a custom_type_resolver kwarg the parametrized signature cannot
# pass, so the sweep goes through CustomEncoder on a known base type instead.
# test_custom_type_resolver_parity covers the real CustomType path.
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


# Each case fails validation on the dict path; the codec path must produce the
# identical (message, instance_path) list. Single-invalid-field cases only, so
# wire order vs field order never matters. Excluded: union all-fail (message
# carries a naming counter, see test_union_all_fail_parity) and
# big-int-for-plain-int (a deliberate divergence).
ERROR_PARITY_CASES: list[tuple[Any, Any]] = [
    (int, '1'),  # wrong scalar type
    (str, 1),
    (list[int], [2, 3, 'foo']),  # array element error with index path
    (dict[str, int], {'foo': 1, 'bar': '2'}),  # dict value error with key path
    (Inner, {'name': 'x'}),  # missing required field
    (Outer, {'inner': {'name': 'x', 'score': 'notfloat'}}),  # nested field error path
    (Annotated[int, Min(10), Max(100)], 1),  # below Min
    (Annotated[int, Min(10), Max(100)], 101),  # above Max
    (int, 1.5),  # float-shaped token for an int field
    (int, 1.0),  # whole-valued float still rejected
    (int, -0.0),
    (Annotated[int, Min(10), Max(100)], 1.5),  # bounded int: same "integer" rejection, not a bounds error
    (Annotated[str, MinLength(6), MaxLength(8)], 'hi'),  # below MinLength
    (Annotated[str, MinLength(6), MaxLength(8)], 'hello world'),  # above MaxLength
    (Color, 'blue'),  # enum invalid value
    (Literal['foo', 'bar'], 1),  # literal invalid value
    (tuple[int, str], [1, 2]),  # tuple element type
    (tuple[int, str], [1]),  # tuple too short
    (tuple[int, str], [1, 'x', 3]),  # tuple too long
]


# ==============================================================================
# Codec-agnostic tests
#
# Parametrized over CODECS via `parametrize_codec` (currently just JSON — adding
# msgpack later means adding one entry to CODECS, not touching these tests).
# Wire bytes for `load()` are built via `_dump_any` and inspected via `_load_any`,
# both routed through the codec's own generic Any bridge, so nothing here assumes
# JSON's specific grammar. Tests that genuinely need JSON's own syntax (raw byte
# literals, escaping, malformed-input recovery, exact dump bytes) live in the
# JSON-specific section below instead.
# ==============================================================================


def test_decode_error_exported():
    assert issubclass(serpyco_rs.DecodeError, ValueError)


def test_codec_is_not_an_extension_point():
    # Format ids live in the Rust core, so a user-defined Codec can never work;
    # fail at class definition with a clear message instead of AttributeError
    # (or `ValueError: unknown format id`) at the first dump.
    with pytest.raises(TypeError, match='not an extension point'):

        class MyCodec(serpyco_rs.Codec):
            pass


@parametrize_codec
def test_dump_codec_roundtrip(codec):
    s = Serializer(Everything)
    data = s.dump(EVERYTHING, codec=codec)
    assert isinstance(data, bytes)
    assert s.load(data, codec=codec) == EVERYTHING


@parametrize_codec
def test_dump_codec_matches_dict_path(codec):
    s = Serializer(Everything)
    assert _load_any(codec, s.dump(EVERYTHING, codec=codec)) == s.dump(EVERYTHING)


@parametrize_codec
def test_codec_in_constructor(codec):
    s = Serializer(Everything, codec=codec)
    data = s.dump(EVERYTHING)
    assert isinstance(data, bytes)
    assert s.load(data) == EVERYTHING


@parametrize_codec
def test_load_codec_accepts_bytes_bytearray_memoryview(codec):
    s = Serializer(Inner, codec=codec)
    raw = s.dump(Inner(name='x', score=1.0))
    for variant in (raw, bytearray(raw), memoryview(raw)):
        assert s.load(variant) == Inner(name='x', score=1.0)


@parametrize_codec
def test_per_call_codec_overrides_instance(codec):
    s = Serializer(Inner)
    assert isinstance(s.dump(Inner(name='x', score=1.0), codec=codec), bytes)
    assert isinstance(s.dump(Inner(name='x', score=1.0)), dict)


@parametrize_codec
def test_big_int_roundtrip(codec):
    s = Serializer(int, codec=codec)
    big = 2**100
    assert s.load(s.dump(big)) == big
    assert _load_any(codec, s.dump(big)) == big


@parametrize_codec
def test_trailing_garbage_on_valid_doc_raises_decode_error(codec):
    s = Serializer(Inner, codec=codec)
    valid = s.dump(Inner(name='x', score=1.0))
    with pytest.raises(serpyco_rs.DecodeError):
        s.load(valid + b' trailing')


@parametrize_codec
def test_schema_invalid_wellformed_raises_schema_error(codec):
    s = Serializer(Inner, codec=codec)
    with pytest.raises(SchemaValidationError):
        s.load(_dump_any(codec, {'a': 1}))


@parametrize_codec
def test_schema_error_has_same_instance_path(codec):
    s = Serializer(Everything)
    good = s.dump(EVERYTHING)
    good['nested'] = {'name': 'x', 'score': 'not a float'}
    with pytest.raises(SchemaValidationError) as dict_err:
        s.load(good)
    with pytest.raises(SchemaValidationError) as codec_err:
        s.load(_dump_any(codec, good), codec=codec)
    assert [(e.message, e.instance_path) for e in codec_err.value.errors] == [
        (e.message, e.instance_path) for e in dict_err.value.errors
    ]


@parametrize_codec
def test_bytes_field_dump_raises(codec):
    s = Serializer(bytes, codec=codec)
    with pytest.raises(ValidationError):
        s.dump(b'raw')


@parametrize_codec
def test_scalar_edges(codec):
    s_int = Serializer(int, codec=codec)
    assert s_int.load(_dump_any(codec, -(2**63))) == -(2**63)

    with pytest.raises(SchemaValidationError):
        s_int.load(_dump_any(codec, 1.5))  # float is not a valid int -> schema error, NOT DecodeError

    s_float = Serializer(float, codec=codec)
    # An integer wire value in a float field returns an int, exactly like the dict
    # path (equality 1 == 1.0 holds; the point is the int type matches the dict path).
    dict_result = Serializer(float).load(1)
    codec_result = s_float.load(_dump_any(codec, 1))
    assert codec_result == dict_result
    assert type(codec_result) is type(dict_result)
    assert s_float.load(_dump_any(codec, 1.5)) == 1.5
    assert type(s_float.load(_dump_any(codec, 1.5))) is float

    s_str = Serializer(str, codec=codec)
    assert s_str.load(s_str.dump('cyrillic sh and \x1f')) == 'cyrillic sh and \x1f'

    s_dt = Serializer(datetime, codec=codec)
    val = datetime(2026, 7, 23, 12, 0, 0, tzinfo=timezone.utc)
    assert s_dt.load(s_dt.dump(val)) == val

    s_dec = Serializer(Decimal, codec=codec)
    assert s_dec.load(_dump_any(codec, '1.100')) == Decimal('1.100')


@parametrize_codec
def test_containers_deep(codec):
    s = Serializer(list[dict[str, list[int]]], codec=codec)
    val = [{'a': [1, 2]}, {'b': []}]
    assert s.load(s.dump(val)) == val

    s2 = Serializer(tuple[int, str, float], codec=codec)
    assert s2.load(s2.dump((1, 'x', 0.5))) == (1, 'x', 0.5)

    s3 = Serializer(dict[str, int], codec=codec)
    assert _load_any(codec, s3.dump({'k': 1})) == {'k': 1}
    assert s3.load(_dump_any(codec, {'k': 1})) == {'k': 1}


@parametrize_codec
def test_array_element_error_path(codec):
    s = Serializer(list[int], codec=codec)
    with pytest.raises(SchemaValidationError) as e:
        s.load(_dump_any(codec, [1, 'bad', 3]))
    # element error must carry the index path, exactly like the dict path
    dict_s = Serializer(list[int])
    with pytest.raises(SchemaValidationError) as d:
        dict_s.load([1, 'bad', 3])
    assert [(x.message, x.instance_path) for x in e.value.errors] == [
        (x.message, x.instance_path) for x in d.value.errors
    ]


@parametrize_codec
def test_entity_unknown_keys_skipped(codec):
    s = Serializer(Inner, codec=codec)
    raw = _dump_any(codec, {'name': 'x', 'unknown': {'deep': [1]}, 'score': 1.0})
    assert s.load(raw) == Inner(name='x', score=1.0)


@parametrize_codec
def test_entity_missing_required_error_parity(codec):
    s = Serializer(Inner)
    sc = Serializer(Inner, codec=codec)
    bad = {'name': 'x'}  # missing score
    with pytest.raises(SchemaValidationError) as d:
        s.load(bad)
    with pytest.raises(SchemaValidationError) as c:
        sc.load(_dump_any(codec, bad))
    assert [(e.message, e.instance_path) for e in c.value.errors] == [
        (e.message, e.instance_path) for e in d.value.errors
    ]


@parametrize_codec
def test_entity_field_error_path_parity(codec):
    s = Serializer(Everything)
    sc = Serializer(Everything, codec=codec)
    bad = s.dump(EVERYTHING)
    bad['nested'] = {'name': 'x', 'score': 'notfloat'}
    with pytest.raises(SchemaValidationError) as d:
        s.load(bad)
    with pytest.raises(SchemaValidationError) as c:
        sc.load(_dump_any(codec, bad))
    assert [(e.message, e.instance_path) for e in c.value.errors] == [
        (e.message, e.instance_path) for e in d.value.errors
    ]


@parametrize_codec
def test_entity_defaults_applied(codec):
    s = Serializer(Everything, codec=codec)
    payload = Serializer(Everything).dump(EVERYTHING)
    del payload['with_default']  # has default=5
    del payload['items']  # has default_factory=list
    obj = s.load(_dump_any(codec, payload))
    assert obj.with_default == 5
    assert obj.items == []


@parametrize_codec
def test_camelcase_codec(codec):
    @dataclass
    class TwoWords:
        long_name: str

    s = Serializer(TwoWords, camelcase_fields=True, codec=codec)
    assert _load_any(codec, s.dump(TwoWords(long_name='a'))) == {'longName': 'a'}
    assert s.load(_dump_any(codec, {'longName': 'a'})) == TwoWords(long_name='a')


@parametrize_codec
def test_omit_none_codec(codec):
    @dataclass
    class WithOpt:
        a: Optional[int] = None
        b: int = 1

    s = Serializer(WithOpt, omit_none=True, codec=codec)
    assert _load_any(codec, s.dump(WithOpt())) == {'b': 1}


@parametrize_codec
def test_typeddict_codec_roundtrip(codec):
    class Movie(TypedDict):
        name: str
        year: int

    s = Serializer(Movie, codec=codec)
    val: Movie = {'name': 'Blade Runner', 'year': 1982}
    assert _load_any(codec, s.dump(val)) == {'name': 'Blade Runner', 'year': 1982}
    assert s.load(_dump_any(codec, val)) == val


@parametrize_codec
def test_typeddict_codec_matches_dict_path(codec):
    class Movie(TypedDict):
        name: str
        year: int

    s = Serializer(Movie)
    sc = Serializer(Movie, codec=codec)
    val: Movie = {'name': 'Blade Runner', 'year': 1982}
    # dump parity
    assert _load_any(codec, sc.dump(val)) == s.dump(val)
    # load parity (including unknown-key skipping)
    raw_dict = {'name': 'x', 'year': 1, 'unknown': [1, 2]}
    assert sc.load(_dump_any(codec, raw_dict)) == s.load(raw_dict)


@parametrize_codec
def test_typeddict_partial_codec_parity(codec):
    class Movie(TypedDict):
        name: str
        year: NotRequired[int]

    s = Serializer(Movie)
    sc = Serializer(Movie, codec=codec)
    val: Movie = {'name': 'x'}  # optional 'year' omitted
    assert _load_any(codec, sc.dump(val)) == s.dump(val)
    assert sc.load(_dump_any(codec, val)) == s.load(dict(val))


@parametrize_codec
def test_flatten_parity_codec(codec):
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
    s_codec = Serializer(Person, codec=codec)
    # dump parity: streaming falls back to the bridge for flatten entities
    assert _load_any(codec, s_codec.dump(person)) == s_dict.dump(person)
    # load parity: native flatten streaming round-trips exactly like the bridge did
    assert s_codec.load(s_codec.dump(person)) == person
    assert s_codec.load(_dump_any(codec, s_dict.dump(person))) == person


@parametrize_codec
def test_flatten_struct_only_parity_codec(codec):
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
    sc = Serializer(Person, codec=codec)
    assert _load_any(codec, sc.dump(person)) == s.dump(person)
    assert sc.load(sc.dump(person)) == person
    assert sc.load(_dump_any(codec, s.dump(person))) == s.load(s.dump(person))


@parametrize_codec
def test_flatten_dict_only_parity_codec(codec):
    @dataclass
    class Person:
        name: str
        extra: Annotated[dict[str, Any], Flatten]

    person = Person(name='John', extra={'phone': '555-1234', 'age': 30})
    s = Serializer(Person)
    sc = Serializer(Person, codec=codec)
    assert _load_any(codec, sc.dump(person)) == s.dump(person)
    assert sc.load(sc.dump(person)) == person
    assert sc.load(_dump_any(codec, s.dump(person))) == s.load(s.dump(person))


@parametrize_codec
def test_flatten_nested_parity_codec(codec):
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
    sc = Serializer(Person, codec=codec)
    assert _load_any(codec, sc.dump(person)) == s.dump(person)
    assert sc.load(sc.dump(person)) == person
    assert sc.load(_dump_any(codec, s.dump(person))) == s.load(s.dump(person))


@parametrize_codec
def test_flatten_missing_optional_default_parity_codec(codec):
    @dataclass
    class Address:
        street: str
        city: str = 'Unknown'

    @dataclass
    class Person:
        name: str
        address: Annotated[Address, Flatten]

    s = Serializer(Person)
    sc = Serializer(Person, codec=codec)
    # 'city' is entirely absent from the wire payload; Address.city default applies.
    raw_dict = {'name': 'John', 'street': '123 Main'}
    raw = _dump_any(codec, raw_dict)
    assert sc.load(raw) == s.load(raw_dict)
    assert sc.load(raw) == Person(name='John', address=Address(street='123 Main', city='Unknown'))


@parametrize_codec
def test_flatten_extra_unknown_keys_parity_codec(codec):
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
    sc = Serializer(Person, codec=codec)
    raw_dict = {'name': 'John', 'street': '123 Main', 'city': 'NYC', 'phone': '555-1234', 'note': 'vip'}
    raw = _dump_any(codec, raw_dict)
    expected = Person(
        name='John',
        address=Address(street='123 Main', city='NYC'),
        extra={'phone': '555-1234', 'note': 'vip'},
    )
    assert sc.load(raw) == expected
    assert sc.load(raw) == s.load(raw_dict)


@parametrize_codec
def test_flatten_nested_dict_catchall_key_overlap_divergence_codec(codec):
    # Pre-existing (not introduced by the codec work) divergence between the two
    # load paths on VALID input, when a nested Flatten struct itself has a dict
    # catch-all: the dict path re-passes the *entire* raw mapping down into
    # `Inner.load`, so a key already claimed by `Outer.a` is also visible to (and
    # lands in) `Inner`'s own catch-all. The streaming codec path routes each wire
    # key to exactly one destination, so `a` is consumed once by `Outer` and never
    # reaches `Inner.rest`. This pins BOTH current behaviors; it does not assert
    # they agree, and is not something to fix here.
    @dataclass
    class Inner:
        x: int
        rest: Annotated[dict[str, Any], Flatten]

    @dataclass
    class Outer:
        a: int
        inner: Annotated[Inner, Flatten]

    s = Serializer(Outer)
    sc = Serializer(Outer, codec=codec)
    raw_dict = {'a': 1, 'x': 2, 'zzz': 3}
    raw = _dump_any(codec, raw_dict)
    assert s.load(raw_dict) == Outer(a=1, inner=Inner(x=2, rest={'a': 1, 'zzz': 3}))
    assert sc.load(raw) == Outer(a=1, inner=Inner(x=2, rest={'zzz': 3}))


@parametrize_codec
def test_flatten_struct_only_extra_key_dropped_parity_codec(codec):
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
    sc = Serializer(Person, codec=codec)
    raw_dict = {'name': 'John', 'street': '123 Main', 'city': 'NYC', 'unexpected': 123}
    raw = _dump_any(codec, raw_dict)
    assert sc.load(raw) == s.load(raw_dict)
    assert sc.load(raw) == Person(name='John', address=Address(street='123 Main', city='NYC'))


@parametrize_codec
def test_flatten_field_error_path_parity_codec(codec):
    @dataclass
    class Address:
        street: str
        city: str

    @dataclass
    class Person:
        name: str
        address: Annotated[Address, Flatten]

    s = Serializer(Person)
    sc = Serializer(Person, codec=codec)
    bad = {'name': 'John', 'street': '123 Main', 'city': 123}
    with pytest.raises(SchemaValidationError) as d:
        s.load(bad)
    with pytest.raises(SchemaValidationError) as c:
        sc.load(_dump_any(codec, bad))
    assert [(e.message, e.instance_path) for e in c.value.errors] == [
        (e.message, e.instance_path) for e in d.value.errors
    ]


@parametrize_codec
def test_flatten_missing_required_error_parity_codec(codec):
    @dataclass
    class Address:
        street: str
        city: str

    @dataclass
    class Person:
        name: str
        address: Annotated[Address, Flatten]

    s = Serializer(Person)
    sc = Serializer(Person, codec=codec)
    bad = {'name': 'John', 'street': '123 Main'}  # city missing entirely
    with pytest.raises(SchemaValidationError) as d:
        s.load(bad)
    with pytest.raises(SchemaValidationError) as c:
        sc.load(_dump_any(codec, bad))
    assert [(e.message, e.instance_path) for e in c.value.errors] == [
        (e.message, e.instance_path) for e in d.value.errors
    ]


@parametrize_codec
def test_flatten_dict_value_error_path_parity_codec(codec):
    @dataclass
    class Person:
        name: str
        extra: Annotated[dict[str, int], Flatten]

    s = Serializer(Person)
    sc = Serializer(Person, codec=codec)
    bad = {'name': 'John', 'age': 'not-an-int'}
    with pytest.raises(SchemaValidationError) as d:
        s.load(bad)
    with pytest.raises(SchemaValidationError) as c:
        sc.load(_dump_any(codec, bad))
    assert [(e.message, e.instance_path) for e in c.value.errors] == [
        (e.message, e.instance_path) for e in d.value.errors
    ]


@parametrize_codec
def test_flatten_wrong_type_error_parity_codec(codec):
    @dataclass
    class Address:
        street: str

    @dataclass
    class Person:
        name: str
        address: Annotated[Address, Flatten]

    s = Serializer(Person)
    sc = Serializer(Person, codec=codec)
    with pytest.raises(SchemaValidationError) as d:
        s.load([1, 2, 3])
    with pytest.raises(SchemaValidationError) as c:
        sc.load(_dump_any(codec, [1, 2, 3]))
    # Only instance_path is compared: the message embeds the raw wire form on the
    # codec path vs a Python repr() on the dict path.
    assert [e.instance_path for e in c.value.errors] == [e.instance_path for e in d.value.errors]
    assert 'not of type "object"' in c.value.errors[0].message
    assert 'not of type "object"' in d.value.errors[0].message


@parametrize_codec
def test_typeddict_flatten_struct_only_parity_codec(codec):
    class Address(TypedDict):
        street: str
        city: str

    class Person(TypedDict):
        name: str
        address: Annotated[Address, Flatten]

    person: Person = {'name': 'John', 'address': {'street': '123 Main', 'city': 'NYC'}}
    s = Serializer(Person)
    sc = Serializer(Person, codec=codec)
    assert _load_any(codec, sc.dump(person)) == s.dump(person)
    assert sc.load(sc.dump(person)) == person
    assert sc.load(_dump_any(codec, s.dump(person))) == s.load(s.dump(person))


@parametrize_codec
def test_typeddict_flatten_dict_only_parity_codec(codec):
    class Person(TypedDict):
        name: str
        extra: Annotated[dict[str, Any], Flatten]

    person: Person = {'name': 'John', 'extra': {'phone': '555-1234', 'age': 30}}
    s = Serializer(Person)
    sc = Serializer(Person, codec=codec)
    assert _load_any(codec, sc.dump(person)) == s.dump(person)
    assert sc.load(sc.dump(person)) == person
    assert sc.load(_dump_any(codec, s.dump(person))) == s.load(s.dump(person))


@parametrize_codec
def test_typeddict_flatten_struct_and_dict_parity_codec(codec):
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
    sc = Serializer(Person, codec=codec)
    assert _load_any(codec, sc.dump(person)) == s.dump(person)
    assert sc.load(sc.dump(person)) == person
    assert sc.load(_dump_any(codec, s.dump(person))) == s.load(s.dump(person))


@parametrize_codec
def test_typeddict_flatten_missing_optional_default_parity_codec(codec):
    class Address(TypedDict):
        street: str
        city: NotRequired[str]

    class Person(TypedDict):
        name: str
        address: Annotated[Address, Flatten]

    s = Serializer(Person)
    sc = Serializer(Person, codec=codec)
    # 'city' is entirely absent from the wire payload; Address.city is
    # NotRequired, so the dict-path default (None) applies.
    raw_dict = {'name': 'John', 'street': '123 Main'}
    raw = _dump_any(codec, raw_dict)
    assert sc.load(raw) == s.load(raw_dict)
    assert sc.load(raw) == {'name': 'John', 'address': {'street': '123 Main', 'city': None}}


@parametrize_codec
def test_union_codec(codec):
    @dataclass
    class P:
        name: str
        score: float

    s = Serializer(int | str | P, codec=codec)
    # primitive members: streaming materializes via parse_any and reuses the load loop
    assert s.load(_dump_any(codec, 5)) == 5
    assert s.load(_dump_any(codec, 'x')) == 'x'
    # container member: raw span is captured and re-parsed by the matching variant
    assert s.load(_dump_any(codec, {'name': 'n', 'score': 1.0})) == P(name='n', score=1.0)
    # discriminator-free container with extra keys still round-trips to the entity
    assert s.load(_dump_any(codec, {'score': 2.5, 'name': 'z'})) == P(name='z', score=2.5)
    # round-trip of primitive members (dataclass dump inside an untagged union is a
    # pre-existing dict-path behavior returning the object unchanged, so it is not
    # serializable via a byte codec; only the streaming load path is in scope here)
    assert s.load(s.dump(5)) == 5
    assert s.load(s.dump('x')) == 'x'


@parametrize_codec
def test_union_all_fail_parity(codec):
    def type_suffix(msg):
        # The message is "<repr of the offending value> is not of type <name>".
        # Only the suffix is comparable across paths: the value repr differs
        # (raw wire bytes on the codec path vs Python repr() on the dict path,
        # same mismatch as test_flatten_wrong_type_error_parity_codec), and the
        # second Serializer built for one type gets a disambiguation counter in
        # the union's name ("int | str" vs "int | str1"); strip both.
        return re.sub(r'\d*"$', '"', msg.split('is not of type', 1)[-1])

    s = Serializer(int | str)
    sc = Serializer(int | str, codec=codec)
    bad = [1, 2, 3]  # neither int nor str -> drives the container all-fail branch
    with pytest.raises(serpyco_rs.SchemaValidationError) as d:
        s.load(bad)
    with pytest.raises(serpyco_rs.SchemaValidationError) as c:
        sc.load(_dump_any(codec, bad))
    assert [e.instance_path for e in c.value.errors] == [e.instance_path for e in d.value.errors]
    assert [type_suffix(e.message) for e in c.value.errors] == [type_suffix(e.message) for e in d.value.errors]


@parametrize_codec
def test_discriminated_union_codec(codec):
    @dataclass
    class Cat:
        kind: Literal['cat']
        meow: str

    @dataclass
    class Dog:
        kind: Literal['dog']
        bark: str

    Pet = Annotated[Union[Cat, Dog], Discriminator('kind')]
    s = Serializer(Pet, codec=codec)
    # discriminator NOT first key -> requires scan-ahead over the captured span
    assert s.load(_dump_any(codec, {'meow': 'm', 'kind': 'cat'})) == Cat(kind='cat', meow='m')
    assert s.load(s.dump(Dog(kind='dog', bark='woof'))) == Dog(kind='dog', bark='woof')

    # unknown tag + missing discriminator: parity with the dict-path
    sd = Serializer(Pet)
    for bad in ({'kind': 'fish', 'meow': 'm'}, {'meow': 'm'}):
        with pytest.raises(serpyco_rs.SchemaValidationError) as d:
            sd.load(bad)
        with pytest.raises(serpyco_rs.SchemaValidationError) as c:
            s.load(_dump_any(codec, bad))
        assert [(e.message, e.instance_path) for e in c.value.errors] == [
            (e.message, e.instance_path) for e in d.value.errors
        ]


@parametrize_codec
def test_untagged_union_dump_entity(codec):
    @dataclass
    class P:
        name: str
        score: float

    # entity is the SECOND member, after a scalar whose dump doesn't validate
    s = Serializer(int | P, codec=codec)
    data = s.dump(P(name='n', score=1.5))
    assert _load_any(codec, data) == {'name': 'n', 'score': 1.5}
    assert s.load(data) == P(name='n', score=1.5)
    # scalar member still dumps correctly
    assert _load_any(codec, s.dump(7)) == 7
    assert s.load(s.dump(7)) == 7


@parametrize_codec
def test_untagged_union_dump_roundtrip_both_orders(codec):
    @dataclass
    class A:
        a: int

    @dataclass
    class B:
        b: str

    s = Serializer(A | B, codec=codec)
    assert s.load(s.dump(A(a=1))) == A(a=1)
    assert s.load(s.dump(B(b='x'))) == B(b='x')


# --- parity sweeps ------------------------------------------------------------
#
# Every case is dumped through both paths (codec output decoded via the generic
# Any bridge vs. the dict-path dump) and round-tripped through the codec.
# Intentional divergences are excluded: big int in a plain int field, untagged-
# union dump, dump-side type coercion.


@parametrize_codec
@pytest.mark.parametrize(('typ', 'value'), PARITY_CASES)
def test_parity_dump(typ, value, codec):
    s = Serializer(typ)
    sc = Serializer(typ, codec=codec)
    assert _load_any(codec, sc.dump(value)) == s.dump(value)


@parametrize_codec
@pytest.mark.parametrize(('typ', 'value'), PARITY_CASES)
def test_parity_roundtrip(typ, value, codec):
    sc = Serializer(typ, codec=codec)
    assert sc.load(sc.dump(value)) == value


@parametrize_codec
@pytest.mark.parametrize(('typ', 'value'), ROUNDTRIP_ONLY_CASES)
def test_parity_roundtrip_only(typ, value, codec):
    sc = Serializer(typ, codec=codec)
    assert sc.load(sc.dump(value)) == value


@parametrize_codec
def test_custom_type_resolver_parity(codec):
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
    sc = Serializer(Data, custom_type_resolver=resolver, codec=codec)
    assert _load_any(codec, sc.dump(val)) == s.dump(val)
    assert sc.load(sc.dump(val)) == val


@parametrize_codec
@pytest.mark.parametrize(('typ', 'bad'), ERROR_PARITY_CASES)
def test_error_parity(typ, bad, codec):
    s = Serializer(typ)
    sc = Serializer(typ, codec=codec)
    with pytest.raises(SchemaValidationError) as dict_err:
        s.load(bad)
    with pytest.raises(SchemaValidationError) as codec_err:
        sc.load(_dump_any(codec, bad))
    assert [(e.message, e.instance_path) for e in codec_err.value.errors] == [
        (e.message, e.instance_path) for e in dict_err.value.errors
    ]


@parametrize_codec
def test_load_rejects_unsupported_input_type(codec):
    # A wrong argument type is a TypeError, not a RuntimeError, so callers can
    # write `except TypeError` around the boundary.
    s = Serializer(int, codec=codec)
    for bad in (123, None, {'a': 1}, [1]):
        with pytest.raises(TypeError):
            s.load(bad)


@parametrize_codec
def test_bound_codec_cannot_be_disabled_per_call(codec):
    # A per-call codec selects a format, it does not switch the dict path back on:
    # `codec=None` is indistinguishable from "argument omitted". Keeping both modes
    # is done by NOT binding a codec and passing it per call (checked below).
    @dataclass
    class M:
        a: int

    bound = Serializer(M, codec=codec)
    disabled = bound.dump(M(a=1), codec=None)
    assert isinstance(disabled, bytes)  # codec=None did not fall back to the dict path
    assert bound.load(disabled, codec=None) == M(a=1)

    # documented way to keep both modes on one serializer
    unbound = Serializer(M)
    assert unbound.dump(M(a=1)) == {'a': 1}
    per_call = unbound.dump(M(a=1), codec=codec)
    assert isinstance(per_call, bytes)
    assert unbound.load({'a': 1}) == M(a=1)
    assert unbound.load(per_call, codec=codec) == M(a=1)


@parametrize_codec
def test_codec_entity_dump_attribute_error_is_chained(codec):
    # Same as the dict path: a broken property must not be silently reinterpreted
    # as "wrong shape" — the original AttributeError stays reachable as __cause__.
    @dataclass
    class Bar:
        a: int
        b: str = ''

    class BadBar:
        a = 1

        @property
        def b(self) -> str:
            raise AttributeError('internal bug in my property')

        def __repr__(self) -> str:
            return 'BadBar()'

    s = Serializer(Bar, codec=codec)
    with pytest.raises(serpyco_rs.SchemaValidationError) as e:
        s.dump(BadBar())

    assert isinstance(e.value.__cause__, AttributeError)
    assert 'internal bug in my property' in str(e.value.__cause__)


@parametrize_codec
def test_codec_int_dump_accepts_int_subclass(codec):
    # An `int` subclass (IntEnum/IntFlag) on an `int` field must dump like the
    # dict path does; only `bool` stays rejected. Mirrors StringEncoder, which
    # already accepts `str` subclasses (StrEnum).
    class Level(IntEnum):
        LOW = 1

    s = Serializer(int, codec=codec)
    assert s.load(s.dump(Level.LOW)) == 1
    with pytest.raises(serpyco_rs.SchemaValidationError):
        s.dump(True)


@parametrize_codec
def test_codec_float_dump_accepts_int_subclass(codec):
    class Level(IntEnum):
        LOW = 1

    s = Serializer(float, codec=codec)
    assert s.load(s.dump(Level.LOW)) == 1


@parametrize_codec
def test_codec_str_dump_accepts_str_subclass(codec):
    # The reference behaviour the int side is being aligned with.
    class Name(str):
        pass

    s = Serializer(str, codec=codec)
    assert s.load(s.dump(Name('x'))) == 'x'


@parametrize_codec
def test_codec_bytes_union_dump_skips_to_next_member(codec):
    # `bytes` before a serializable member must not abort union probing.
    s = Serializer(Union[bytes, str], codec=codec)
    assert isinstance(s.dump('x'), bytes)
    assert s.load(s.dump('x')) == 'x'
    # a genuine bytes value still errors clearly (no serializable member)
    with pytest.raises(serpyco_rs.ValidationError):
        s.dump(b'x')


@parametrize_codec
def test_codec_dict_omit_none_validates_key(codec):
    # under omit_none, a skipped None value must not mask key validation.
    s = Serializer(dict[Color, Optional[int]], codec=codec, omit_none=True)
    sd = Serializer(dict[Color, Optional[int]], omit_none=True)
    bad = {'not_a_color': None}
    with pytest.raises(serpyco_rs.SchemaValidationError):
        sd.dump(bad)  # dict path validates the key
    with pytest.raises(serpyco_rs.SchemaValidationError):
        s.dump(bad)  # codec must too (currently returns an empty object)
    # valid case still omits None values and dumps present ones
    assert s.load(s.dump({Color.RED: None})) == {}
    assert _load_any(codec, s.dump({Color.RED: 5})) == {'red': 5}


@parametrize_codec
def test_union_kind_narrowing_reads_off_the_cursor(codec):
    # A union whose members occupy distinct wire kinds is resolved by the kind
    # alone: the sole viable member reads straight off the cursor (no skip pass,
    # no re-parse), and every member still round-trips.
    @dataclass
    class P:
        name: str
        score: float

    s = Serializer(Union[P, str], codec=codec)
    assert s.load(_dump_any(codec, {'name': 'n', 'score': 1.5})) == P(name='n', score=1.5)
    assert s.load(_dump_any(codec, 'plain')) == 'plain'

    # Ambiguous kinds (both members accept a number) keep the probing path.
    si = Serializer(Union[int, float], codec=codec)
    assert si.load(_dump_any(codec, 7)) == 7
    assert si.load(_dump_any(codec, 1.5)) == 1.5


@parametrize_codec
def test_union_kind_narrowing_error_is_the_member_error(codec):
    # With one viable member, its error surfaces as-is: the path points at the
    # offending field instead of the union root. This is a deliberate divergence
    # from the dict path, which reports "nothing matched" at the root.
    @dataclass
    class P:
        foo: int

    sc = Serializer(Union[int, P], codec=codec)
    with pytest.raises(serpyco_rs.SchemaValidationError) as c:
        sc.load(_dump_any(codec, {'foo': 'not-an-int'}))
    (err,) = c.value.errors
    assert err.instance_path == 'foo'
    assert 'is not of type "integer"' in err.message

    # No member accepts the kind -> the union's own error at the root.
    with pytest.raises(serpyco_rs.SchemaValidationError) as c:
        sc.load(_dump_any(codec, [1]))
    (err,) = c.value.errors
    assert err.instance_path == ''
    assert 'is not of type' in err.message


@parametrize_codec
def test_union_kind_narrowing_keeps_optional_and_nested_members(codec):
    # Optional accepts null on top of whatever it wraps, so `None` stays viable
    # next to a same-kind member instead of being narrowed away.
    @dataclass
    class P:
        foo: int

    s = Serializer(Union[Optional[P], str], codec=codec)
    assert s.load(_dump_any(codec, None)) is None
    assert s.load(_dump_any(codec, {'foo': 1})) == P(foo=1)
    assert s.load(_dump_any(codec, 's')) == 's'


@parametrize_codec
def test_entity_load_key_order_independent(codec):
    # The streaming load path guesses that keys arrive in schema order; a shuffled
    # or unknown-key-interleaved document must load identically. (A literal
    # *duplicate* key can't be built from a Python dict — see
    # test_entity_load_duplicate_key_last_wins in the JSON-specific section.)
    @dataclass
    class M:
        a: int
        b: str
        c: bool
        d: Optional[int] = None

    s = Serializer(M, codec=codec)
    sd = Serializer(M)
    for raw_dict in (
        {'a': 1, 'b': 'x', 'c': True, 'd': 2},  # schema order
        {'d': 2, 'c': True, 'b': 'x', 'a': 1},  # reversed
        {'b': 'x', 'a': 1, 'd': 2, 'c': True},  # shuffled
        {'a': 1, 'zzz': [1, {'k': 2}], 'b': 'x', 'c': True, 'd': 2},  # unknown key between
    ):
        assert s.load(_dump_any(codec, raw_dict)) == sd.load(raw_dict) == M(a=1, b='x', c=True, d=2)

    # missing trailing field falls back to its default
    raw = _dump_any(codec, {'a': 1, 'b': 'x', 'c': False})
    assert s.load(raw) == M(a=1, b='x', c=False, d=None)


# ==============================================================================
# JSON-specific tests
#
# These test JSON's own wire grammar directly: raw byte literals, malformed-
# input recovery, string-escape decoding, or an exact dump byte string. None of
# that generalizes to another format, so it is left hardcoded to JSON rather
# than forced through the _dump_any/_load_any helpers above.
# ==============================================================================


@pytest.mark.parametrize(('typ', 'value'), PARITY_CASES)
def test_parity_dump_json_oracle(typ, value):
    # test_parity_dump (codec-agnostic section) checks codec-dump == dict-dump
    # by decoding both sides through `_load_any`, which routes through the same
    # low-level Parser/Writer primitives (take_str_known, write_f64, ...) the
    # typed encoders use. That makes it a self-consistency check, not an
    # independent one: a bug shared by both sides of a primitive (e.g. string
    # escaping, float formatting) couldn't be caught there. Re-run the same
    # comparison here with `json.loads` — CPython's independently implemented
    # JSON parser — standing in for `_load_any`, across the same PARITY_CASES
    # matrix, so the whole type matrix still has an independent oracle for at
    # least one codec. (This also covers the single EVERYTHING/Everything case
    # from test_dump_codec_matches_dict_path, which is one of PARITY_CASES.)
    s = Serializer(typ)
    sc = Serializer(typ, codec=JSON)
    assert json.loads(sc.dump(value)) == s.dump(value)


def test_malformed_json_raises_decode_error():
    s = Serializer(Inner, codec=JSON)
    for bad in (b'{', b'', b'{"name": "x", "score": }', b'[1,]'):
        with pytest.raises(serpyco_rs.DecodeError):
            s.load(bad)


def test_nan_raises():
    # NaN/Infinity have no representation in the JSON grammar; a future binary
    # codec's float type could legitimately accept them (IEEE-754 doubles do),
    # so this rejection is a JSON-specific constraint, not a general one.
    s = Serializer(float, codec=JSON)
    with pytest.raises(ValidationError):
        s.dump(float('nan'))


def test_scalar_edges_json_specific():
    # Pieces of the scalar-edges sweep that are inherently about JSON's textual
    # grammar and cannot be phrased through another format's wire form.
    s_str = Serializer(str, codec=JSON)
    # Backslash-n and backslash-u0000 are JSON string-escape syntax; a binary
    # codec's strings are raw UTF-8 with no such escape grammar to decode.
    assert s_str.load('"a\\nb\\u0000"'.encode()) == 'a\nb\x00'

    s_dec = Serializer(Decimal, codec=JSON)
    # An *unquoted* JSON number token: precision comes straight from the raw
    # source text, not through a lossy float64 round-trip. A binary codec has no
    # raw text to preserve for a number in the first place.
    assert s_dec.load(b'1.1') == Decimal('1.1')  # precision from raw text, not repr(float)


def test_recursion_depth_codec():
    # The nested-array bytes are crafted directly (not via dump) because dump
    # enforces the same recursion guard the load path is being tested here, so
    # going through dump first would fail before load is ever reached.
    s = Serializer(Any, codec=JSON)
    deep = b'[' * 2000 + b'1' + b']' * 2000
    with pytest.raises(RecursionError):
        s.load(deep)


def test_load_json_accepts_str():
    # `load` also accepts `str`: its UTF-8 bytes are reinterpreted as the wire
    # buffer directly. That is only meaningful for a text-based format — JSON's
    # own dump output is valid UTF-8 and round-trips through str, but a binary
    # codec's bytes would generally not decode as str at all.
    s = Serializer(Inner, codec=JSON)
    raw = s.dump(Inner(name='x', score=1.0))
    assert s.load(raw.decode()) == Inner(name='x', score=1.0)


# IntEncoder::load_format's float branch never accepts a float for an int field;
# it re-parses the raw number purely to render the same "... is not of type
# "integer"" message as the dict path (see the comment on that branch). These
# cases use raw wire bytes rather than `json.dumps(python_value)` (or the
# `_dump_any` helper, which goes through the same dump-side float writer) because
# that round-trip would normalize the exponent/precision away (e.g. `1e3` becomes
# `1000.0`) or reject the value outright (`1e400` overflows to an Infinity dump
# can't produce), stopping this from testing the wire form at all.
@pytest.mark.parametrize(
    ('raw', 'equivalent_value'),
    [
        (b'1e3', 1e3),  # exponent notation on the wire, not just a plain decimal
        (b'1e400', float('inf')),  # overflows to +inf, same as Python's own float parse
        (b'-1e400', float('-inf')),
        (b'1e-400', 0.0),  # underflows to 0.0
        (b'1.' + b'1' * 400, float('1.' + '1' * 400)),  # very long mantissa
    ],
)
def test_int_rejects_float_wire_forms(raw, equivalent_value):
    s = Serializer(int)
    sc = Serializer(int, codec=JSON)
    with pytest.raises(SchemaValidationError) as dict_err:
        s.load(equivalent_value)
    with pytest.raises(SchemaValidationError) as codec_err:
        sc.load(raw)
    assert [(e.message, e.instance_path) for e in codec_err.value.errors] == [
        (e.message, e.instance_path) for e in dict_err.value.errors
    ]


def test_union_all_fail_message_json_specific():
    # test_union_all_fail_parity (codec-agnostic section) strips the offending
    # value's rendering before comparing, because the two paths render a
    # container differently: raw wire text on the codec path, Python repr() on
    # the dict path. Pin both renderings literally here — this is the actual
    # observed divergence, not the intended contract, so it is not fixed. The
    # trailing digit `strip_counter` removes is the same pre-existing
    # per-Serializer union-naming-counter artifact test_union_all_fail_parity
    # already strips (a second `Serializer(int | str, ...)` built anywhere in
    # the process gets "int | str1", "int | str2", ... to disambiguate).
    def strip_counter(msg):
        return re.sub(r'\d*"$', '"', msg)

    s = Serializer(int | str)
    sc = Serializer(int | str, codec=JSON)
    bad = [1, 2, 3]
    with pytest.raises(serpyco_rs.SchemaValidationError) as d:
        s.load(bad)
    with pytest.raises(serpyco_rs.SchemaValidationError) as c:
        sc.load(_dump_any(JSON, bad))
    assert strip_counter(d.value.errors[0].message) == '[1, 2, 3] is not of type "int | str"'
    assert strip_counter(c.value.errors[0].message) == '[1,2,3] is not of type "int | str"'


# --- malformed-input (DecodeError) corpus -------------------------------------
#
# Loaded through a permissive `Any` type so only the syntax can fail.


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
        b'x',  # bare token: not a digit/'-' lead byte -> parser.peek() must reject it explicitly
        b'Infinity',  # jiter recognizes this lead byte but we never enable allow_inf_nan
        b'NaN',
    ],
)
def test_decode_error_corpus(bad):
    s = Serializer(Any, codec=JSON)  # permissive: only JSON syntax can fail
    with pytest.raises(serpyco_rs.DecodeError) as e:
        s.load(bad)
    assert isinstance(e.value.position, int)


def test_codec_literal_bool_roundtrip():
    # bool-valued Literal: the codec must be able to decode its own dump output.
    # Byte-exact: `true`/`false` are JSON keyword literals, not a general
    # "dump a bool" concern (a binary codec would use its own boolean encoding).
    s = Serializer(Literal[True, False], codec=JSON)
    sd = Serializer(Literal[True, False])
    assert s.dump(True) == b'true'
    assert s.dump(False) == b'false'
    assert s.load(b'true') is True
    assert s.load(b'false') is False
    assert s.load(b'true') is sd.load(True)  # parity with dict path


def test_codec_float_dump_rejects_bool():
    # dict-path dump is lenient (orjson emits `true`); the codec dump validates
    # types and must not silently reinterpret a bool as the integer 1. Byte-exact
    # assertions below tie this to JSON's own textual number formatting.
    s = Serializer(float, codec=JSON)
    with pytest.raises(serpyco_rs.SchemaValidationError):
        s.dump(True)
    # a plain int is still accepted for a float field
    assert s.dump(2) == b'2'
    assert json.loads(s.dump(1.5)) == 1.5


def test_json_dump_is_compact():
    # The JSON writer emits no whitespace between tokens. Verified once, here,
    # with an exact byte comparison — not generalizable, since a binary codec
    # has no "whitespace" concept for its dump output at all.
    @dataclass
    class M:
        a: int

    assert Serializer(M, codec=JSON).dump(M(a=1)) == b'{"a":1}'


def test_entity_load_duplicate_key_last_wins():
    # A literal repeated key on the wire cannot be represented as a Python dict
    # (the dict collapses duplicates before any codec sees it), so this needs a
    # raw JSON literal rather than the codec-agnostic dict-based sweep in
    # test_entity_load_key_order_independent above.
    @dataclass
    class M:
        a: int
        b: str
        c: bool
        d: Optional[int] = None

    s = Serializer(M, codec=JSON)
    assert s.load(b'{"a": 9, "b": "x", "c": true, "a": 1, "d": 2}') == M(a=1, b='x', c=True, d=2)
