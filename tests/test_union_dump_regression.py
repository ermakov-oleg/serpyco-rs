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
