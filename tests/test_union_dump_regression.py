import datetime
from dataclasses import dataclass
from typing import Optional, TypedDict, Union

import pytest

from serpyco_rs import JSON, Serializer


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


@pytest.mark.parametrize('codec', [None, JSON])
def test_dump_list_of_union_of_dataclasses_dispatches_by_runtime_type(codec):
    # Regression (1.21.0): dumping a list[Attach | Create] always went through the
    # first member's encoder, so a Create raised AttributeError: no attribute 'id'.
    @dataclass
    class Embedded:
        contacts: Optional[list[Union[Attach, Create]]] = None

    s = Serializer(Embedded, codec=codec)
    dumped = s.dump(Embedded(contacts=[Attach(id=1), Create(name='x')]))
    if codec is not None:
        assert dumped == b'{"contacts":[{"id":1,"is_main":null},{"name":"x"}]}'
    else:
        assert dumped == {'contacts': [{'id': 1, 'is_main': None}, {'name': 'x'}]}


@pytest.mark.parametrize('codec', [None, JSON])
def test_dump_list_of_union_of_dataclasses_reversed_member_order(codec):
    @dataclass
    class Embedded:
        contacts: Optional[list[Union[Create, Attach]]] = None

    s = Serializer(Embedded, codec=codec)
    dumped = s.dump(Embedded(contacts=[Attach(id=2, is_main=True)]))
    if codec is not None:
        assert dumped == b'{"contacts":[{"id":2,"is_main":true}]}'
    else:
        assert dumped == {'contacts': [{'id': 2, 'is_main': True}]}


@pytest.mark.parametrize('codec', [None, JSON])
def test_dump_union_of_typed_dicts_skips_member_with_missing_required_key(codec):
    # Regression: a missing required key on the tried TypedDict member is a type
    # mismatch (try the next member), not a hard ValidationError that aborts the dump.
    class TdA(TypedDict):
        a: int

    class TdB(TypedDict):
        b: str

    s = Serializer(Union[TdA, TdB], codec=codec)
    if codec is not None:
        assert s.dump({'a': 1}) == b'{"a":1}'
        assert s.dump({'b': 'x'}) == b'{"b":"x"}'
    else:
        assert s.dump({'a': 1}) == {'a': 1}
        assert s.dump({'b': 'x'}) == {'b': 'x'}
