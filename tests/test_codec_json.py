import pytest

import serpyco_rs


def test_decode_error_exported():
    assert issubclass(serpyco_rs.DecodeError, ValueError)


import uuid
from dataclasses import dataclass, field
from datetime import date, datetime, time, timezone
from decimal import Decimal
from enum import Enum
from typing import Any, Optional

import orjson

from serpyco_rs import JSON, Serializer


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


def test_dump_codec_roundtrip():
    s = Serializer(Everything)
    data = s.dump(EVERYTHING, codec=JSON)
    assert isinstance(data, bytes)
    assert s.load(data, codec=JSON) == EVERYTHING


def test_dump_codec_matches_dict_path():
    s = Serializer(Everything)
    assert orjson.loads(s.dump(EVERYTHING, codec=JSON)) == s.dump(EVERYTHING)


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
    assert orjson.loads(s.dump(big)) == big


def test_malformed_json_raises_decode_error():
    import serpyco_rs

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
    from serpyco_rs import SchemaValidationError

    s = Serializer(Inner, codec=JSON)
    with pytest.raises(SchemaValidationError):
        s.load(b'{"a": 1}')


def test_schema_error_has_same_instance_path():
    from serpyco_rs import SchemaValidationError

    s = Serializer(Everything)
    good = s.dump(EVERYTHING)
    good['nested'] = {'name': 'x', 'score': 'not a float'}
    import pytest

    with pytest.raises(SchemaValidationError) as dict_err:
        s.load(good)
    with pytest.raises(SchemaValidationError) as codec_err:
        s.load(orjson.dumps(good), codec=JSON)
    assert [(e.message, e.instance_path) for e in codec_err.value.errors] == [
        (e.message, e.instance_path) for e in dict_err.value.errors
    ]


def test_bytes_field_json_dump_raises():
    from serpyco_rs import ValidationError
    import pytest

    s = Serializer(bytes, codec=JSON)
    with pytest.raises(ValidationError):
        s.dump(b'raw')


def test_nan_raises():
    from serpyco_rs import ValidationError
    import pytest

    s = Serializer(float, codec=JSON)
    with pytest.raises(ValidationError):
        s.dump(float('nan'))


def test_scalar_edges():
    import pytest
    from datetime import datetime, timezone
    from decimal import Decimal

    s_int = Serializer(int, codec=JSON)
    assert s_int.load(b'-9223372036854775808') == -(2**63)
    from serpyco_rs import SchemaValidationError

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
    assert orjson.loads(s3.dump({'k': 1})) == {'k': 1}
    assert s3.load(b'{"k": 1}') == {'k': 1}


def test_array_element_error_path():
    from serpyco_rs import SchemaValidationError

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
    from typing import Any

    s = Serializer(Any, codec=JSON)
    deep = b'[' * 2000 + b'1' + b']' * 2000
    with pytest.raises(RecursionError):
        s.load(deep)


def test_entity_unknown_keys_skipped():
    s = Serializer(Inner, codec=JSON)
    assert s.load(b'{"name": "x", "unknown": {"deep": [1]}, "score": 1.0}') == Inner(name='x', score=1.0)


def test_entity_missing_required_error_parity():
    from serpyco_rs import SchemaValidationError

    s = Serializer(Inner)
    sc = Serializer(Inner, codec=JSON)
    bad = {'name': 'x'}  # missing score
    with pytest.raises(SchemaValidationError) as d:
        s.load(bad)
    with pytest.raises(SchemaValidationError) as c:
        sc.load(orjson.dumps(bad))
    assert [(e.message, e.instance_path) for e in c.value.errors] == [(e.message, e.instance_path) for e in d.value.errors]


def test_entity_field_error_path_parity():
    from serpyco_rs import SchemaValidationError

    s = Serializer(Everything)
    sc = Serializer(Everything, codec=JSON)
    bad = s.dump(EVERYTHING)
    bad['nested'] = {'name': 'x', 'score': 'notfloat'}
    with pytest.raises(SchemaValidationError) as d:
        s.load(bad)
    with pytest.raises(SchemaValidationError) as c:
        sc.load(orjson.dumps(bad))
    assert [(e.message, e.instance_path) for e in c.value.errors] == [(e.message, e.instance_path) for e in d.value.errors]


def test_entity_defaults_applied():
    s = Serializer(Everything, codec=JSON)
    payload = Serializer(Everything).dump(EVERYTHING)
    del payload['with_default']  # has default=5
    del payload['items']  # has default_factory=list
    obj = s.load(orjson.dumps(payload))
    assert obj.with_default == 5
    assert obj.items == []


def test_camelcase_codec():
    @dataclass
    class TwoWords:
        long_name: str

    s = Serializer(TwoWords, camelcase_fields=True, codec=JSON)
    assert orjson.loads(s.dump(TwoWords(long_name='a'))) == {'longName': 'a'}
    assert s.load(b'{"longName": "a"}') == TwoWords(long_name='a')


def test_omit_none_codec():
    @dataclass
    class WithOpt:
        a: Optional[int] = None
        b: int = 1

    s = Serializer(WithOpt, omit_none=True, codec=JSON)
    assert orjson.loads(s.dump(WithOpt())) == {'b': 1}


def test_typeddict_codec_roundtrip():
    from typing_extensions import TypedDict

    class Movie(TypedDict):
        name: str
        year: int

    s = Serializer(Movie, codec=JSON)
    val: Movie = {'name': 'Blade Runner', 'year': 1982}
    assert orjson.loads(s.dump(val)) == {'name': 'Blade Runner', 'year': 1982}
    assert s.load(orjson.dumps(val)) == val


def test_typeddict_codec_matches_dict_path():
    from typing_extensions import TypedDict

    class Movie(TypedDict):
        name: str
        year: int

    s = Serializer(Movie)
    sc = Serializer(Movie, codec=JSON)
    val: Movie = {'name': 'Blade Runner', 'year': 1982}
    # dump parity
    assert orjson.loads(sc.dump(val)) == s.dump(val)
    # load parity (including unknown-key skipping)
    raw = b'{"name": "x", "year": 1, "unknown": [1, 2]}'
    assert sc.load(raw) == s.load(orjson.loads(raw))


def test_typeddict_partial_codec_parity():
    from typing_extensions import NotRequired, TypedDict

    class Movie(TypedDict):
        name: str
        year: NotRequired[int]

    s = Serializer(Movie)
    sc = Serializer(Movie, codec=JSON)
    val: Movie = {'name': 'x'}  # optional 'year' omitted
    assert orjson.loads(sc.dump(val)) == s.dump(val)
    assert sc.load(orjson.dumps(val)) == s.load(dict(val))


def test_flatten_parity_codec():
    from typing import Annotated

    from serpyco_rs.metadata import Flatten

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
    assert orjson.loads(s_codec.dump(person)) == s_dict.dump(person)
    # load parity: round-trips through the bridge fallback
    assert s_codec.load(s_codec.dump(person)) == person
    assert s_codec.load(orjson.dumps(s_dict.dump(person))) == person


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
    import re

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
        sc.load(orjson.dumps(bad))
    assert norm(c.value.errors) == norm(d.value.errors)


def test_discriminated_union_codec():
    from typing import Annotated, Literal, Union

    from serpyco_rs.metadata import Discriminator

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
            s.load(orjson.dumps(bad))
        assert [(e.message, e.instance_path) for e in c.value.errors] == [
            (e.message, e.instance_path) for e in d.value.errors
        ]


def test_untagged_union_dump_entity():
    from dataclasses import dataclass
    import orjson
    @dataclass
    class P:
        name: str
        score: float
    # entity is the SECOND member, after a scalar whose dump doesn't validate
    s = Serializer(int | P, codec=JSON)
    data = s.dump(P(name='n', score=1.5))
    assert orjson.loads(data) == {'name': 'n', 'score': 1.5}
    assert s.load(data) == P(name='n', score=1.5)
    # scalar member still dumps correctly
    assert orjson.loads(s.dump(7)) == 7
    assert s.load(s.dump(7)) == 7


def test_untagged_union_dump_roundtrip_both_orders():
    from dataclasses import dataclass
    import orjson
    @dataclass
    class A:
        a: int
    @dataclass
    class B:
        b: str
    s = Serializer(A | B, codec=JSON)
    assert s.load(s.dump(A(a=1))) == A(a=1)
    assert s.load(s.dump(B(b='x'))) == B(b='x')
