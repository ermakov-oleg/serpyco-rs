import datetime
from dataclasses import dataclass
from typing import Union

from serpyco_rs import Serializer


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
    # Regression: dict-path union dump must skip a structurally non-matching
    # dataclass member. A missing attribute on the tried member is a type
    # mismatch (try the next member), not a hard AttributeError that aborts.
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
