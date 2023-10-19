import orjson
import pytest

from .compare.libs import serpyco_rs as serializer
from .utils import repeat


def test_dump(benchmark):
    serializer.dump(serializer.test_object)  # warmup
    benchmark.group = 'dump full'
    benchmark(repeat(lambda: serializer.dump(serializer.test_object), count=100))


def test_load_validate(benchmark):
    test_dict = serializer.dump(serializer.test_object)
    serializer.load(test_dict)  # warmup

    benchmark.group = 'load full with validate'
    benchmark(repeat(lambda: serializer.load(test_dict), count=100))
