import gc
from dataclasses import dataclass
from enum import Enum

import serpyco_rs


def test_repr_ref_count_segfault_on_default_value():
    class Bar(Enum):
        a = 'a'

    @dataclass
    class Foo:
        lang: Bar = Bar.a

    serializer = serpyco_rs.Serializer(Foo)

    for i in range(10):
        serializer.load({})
        serializer.load_json('{}')
        gc.collect()
