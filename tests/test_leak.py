import gc
import sys
from dataclasses import dataclass
from decimal import Decimal
from enum import Enum

import serpyco_rs


def test_repr_ref_count_segfault_on_default_value():
    class Bar(Enum):
        a = 'a'

    @dataclass
    class Foo:
        lang: Bar = Bar.a

    serializer = serpyco_rs.Serializer(Foo)

    for _i in range(10):
        serializer.load({})
        gc.collect()


def test_load_rc_count_decimal():
    serializer = serpyco_rs.Serializer(Decimal)

    val = '123.1'

    ref_count = sys.getrefcount(val)

    for _i in range(100):
        serializer.load(val)
        gc.collect()

    assert ref_count == sys.getrefcount(val)


def test_dump_rc_count_decimal():
    serializer = serpyco_rs.Serializer(Decimal)

    val = Decimal('123.1')

    ref_count = sys.getrefcount(val)

    for _i in range(100):
        serializer.dump(val)
        gc.collect()

    assert ref_count == sys.getrefcount(val)
