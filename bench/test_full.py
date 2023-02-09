from .compare.libs import serpyco_rs as serializer
from .utils import repeat


def test_dump(benchmark):
    serializer.dump(serializer.test_object)  # warmup
    benchmark.group = "dump"
    benchmark(repeat(lambda: serializer.dump(serializer.test_object), count=100))


def test_load(benchmark):
    test_dict = serializer.dump(serializer.test_object)
    serializer.load(test_dict, validate=False)  # warmup

    benchmark.group = "load"
    benchmark(repeat(lambda: serializer.load(test_dict, validate=False), count=100))


def test_load_validate(benchmark):
    test_dict = serializer.dump(serializer.test_object)
    serializer.load(test_dict, validate=True)  # warmup

    benchmark.group = "load with validate"
    benchmark(repeat(lambda: serializer.load(test_dict, validate=True), count=100))
