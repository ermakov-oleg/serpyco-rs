import datetime
from dataclasses import dataclass
from typing import Optional, TypedDict, Union

import pytest

from serpyco_rs import SchemaValidationError, Serializer, ValidationError
from serpyco_rs._impl import ErrorItem

from tests._codecs import as_python, parametrize_codec_or_dict


def test_dump_union_time_or_date_picks_right_encoder():
    @dataclass
    class Foo:
        val: Union[datetime.time, datetime.date]

    s = Serializer(Foo)
    result = s.dump(Foo(val=datetime.date(2024, 1, 1)))
    assert result['val'] == '2024-01-01'

    result = s.dump(Foo(val=datetime.time(10, 30)))
    assert result['val'].startswith('10:30')


def test_dump_union_of_dataclasses_skips_non_matching_member():
    # Regression: a missing attribute on the tried member is a type mismatch (try the
    # next member), not a hard AttributeError that aborts the whole union dump.
    @dataclass
    class A:
        a: int

    @dataclass
    class B:
        b: str

    @dataclass
    class Wrap:
        val: Union[A, B]

    s = Serializer(Wrap)
    assert s.dump(Wrap(val=A(a=1))) == {'val': {'a': 1}}
    assert s.dump(Wrap(val=B(b='x'))) == {'val': {'b': 'x'}}


@dataclass
class Attach:
    id: Optional[int] = None
    is_main: Optional[bool] = None


@dataclass
class Create:
    name: Optional[str] = None


@dataclass
class AttachFirst:
    contacts: Optional[list[Union[Attach, Create]]] = None


@dataclass
class CreateFirst:
    contacts: Optional[list[Union[Create, Attach]]] = None


class TdA(TypedDict):
    a: int


class TdB(TypedDict):
    b: str


class SharedA(TypedDict):
    shared: int
    only_a: int


class SharedB(TypedDict):
    shared: int
    only_b: str


@parametrize_codec_or_dict
@pytest.mark.parametrize('model', [AttachFirst, CreateFirst], ids=['attach-first', 'create-first'])
def test_dump_list_of_union_of_dataclasses_dispatches_by_runtime_type(codec, model):
    # Regression (1.21.0): dumping a list[Attach | Create] always went through the
    # first member's encoder, so a Create raised AttributeError: no attribute 'id'.
    s = Serializer(model, codec=codec)
    dumped = s.dump(model(contacts=[Attach(id=1), Create(name='x')]))
    assert as_python(codec, dumped) == {'contacts': [{'id': 1, 'is_main': None}, {'name': 'x'}]}


@parametrize_codec_or_dict
@pytest.mark.parametrize('value', [{'a': 1}, {'b': 'x'}], ids=['first-member', 'second-member'])
def test_dump_union_of_typed_dicts_skips_member_with_missing_required_key(codec, value):
    # Regression: a missing required key on the tried TypedDict member is a type
    # mismatch (try the next member), not a hard ValidationError that aborts the dump.
    s = Serializer(Union[TdA, TdB], codec=codec)
    assert as_python(codec, s.dump(value)) == value


@parametrize_codec_or_dict
def test_dump_union_of_typed_dicts_rolls_back_partially_written_member(codec):
    # The mismatch surfaces only on the member's second key, so `shared` is already in
    # the codec's buffer — under a map header msgpack has to backpatch — by the time the
    # member is abandoned. Nothing of the skipped attempt may survive in the output.
    s = Serializer(list[Union[SharedA, SharedB]], codec=codec)
    value = [{'shared': 1, 'only_a': 2}, {'shared': 3, 'only_b': 'x'}]
    assert as_python(codec, s.dump(value)) == value


@parametrize_codec_or_dict
def test_dump_typed_dict_missing_required_key_outside_union_reports_schema_error(codec):
    # With no union to absorb it the mismatch still surfaces, now as
    # SchemaValidationError. It subclasses ValidationError, so `except` clauses written
    # against the ValidationError this used to raise keep catching it.
    s = Serializer(TdA, codec=codec)
    with pytest.raises(SchemaValidationError) as exc:
        s.dump({})
    assert isinstance(exc.value, ValidationError)
    assert exc.value.errors == [ErrorItem(message='data dictionary is missing required parameter a', instance_path='')]
