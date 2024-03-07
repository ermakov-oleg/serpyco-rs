import pytest

import serpyco_rs

from .compare.libs import serpyco_rs as serializer
from .utils import repeat


def test_dump(bench_or_check_refcount):
    serializer.dump(serializer.test_object)  # warmup
    bench_or_check_refcount.group = 'dump full'
    bench_or_check_refcount(repeat(lambda: serializer.dump(serializer.test_object), count=100))


def test_load_validate(bench_or_check_refcount):
    test_dict = serializer.dump(serializer.test_object)
    serializer.load(test_dict)  # warmup

    bench_or_check_refcount.group = 'load full with validate'
    bench_or_check_refcount(repeat(lambda: serializer.load(test_dict), count=100))


@pytest.mark.skip(reason='Need fix refcount leak')
def test_create_serializer(bench_or_check_refcount):
    bench_or_check_refcount.group = 'create serializer'
    bench_or_check_refcount(repeat(lambda: serpyco_rs.Serializer(serializer.Dataclass), count=100))
