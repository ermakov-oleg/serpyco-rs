import orjson
import pytest

from .compare.libs import serpyco_rs as serializer
from .utils import repeat


def test_dump(benchmark):
    serializer.dump(serializer.test_object)  # warmup
    benchmark.group = 'dump full'
    benchmark(repeat(lambda: serializer.dump(serializer.test_object), count=100))


def test_load(benchmark):
    test_dict = serializer.dump(serializer.test_object)
    serializer.load(test_dict, validate=False)  # warmup

    benchmark.group = 'load full'
    benchmark(repeat(lambda: serializer.load(test_dict, validate=False), count=100))


def test_load_validate(benchmark):
    test_dict = serializer.dump(serializer.test_object)
    serializer.load(test_dict, validate=True)  # warmup

    benchmark.group = 'load full with validate'
    benchmark(repeat(lambda: serializer.load(test_dict, validate=True), count=100))


@pytest.mark.parametrize('raw', [True, False], ids=['raw', 'python'])
def test_load_validate_compare(benchmark, raw):
    raw_data = orjson.dumps(serializer.dump(serializer.test_object)).decode('utf-8')

    serializer.load(orjson.loads(raw_data), validate=True)  # warmup
    serializer.load_json(raw_data, validate=True)  # warmup

    def callback():
        return serializer.load(orjson.loads(raw_data), validate=True)

    def raw_callback():
        return serializer.load_json(raw_data, validate=True)

    benchmark.group = 'load with validate (compare)'
    benchmark(repeat(raw_callback if raw else callback, count=100))
