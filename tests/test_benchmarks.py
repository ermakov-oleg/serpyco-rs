import dataclasses
import typing

import pytest
import serpyco
import serpyco_rs


@dataclasses.dataclass
class Nested(object):
    """
    A nested type for Dataclass
    """

    name: str


@dataclasses.dataclass
class Dataclass(object):
    """
    A Dataclass class
    """

    name: str
    value: int
    f: float
    b: bool
    nest: typing.List[Nested]
    many: typing.List[int]
    option: typing.Optional[str] = None


serializer_cython = serpyco.Serializer(Dataclass)
serializer_rs = serpyco_rs.Serializer(Dataclass)

test_object = Dataclass(
    name="Foo",
    value=42,
    f=12.34,
    b=True,
    nest=[Nested(name="Bar_{}".format(index)) for index in range(0, 1000)],
    many=[1, 2, 3],
)


serializers = {
    "cython": serializer_cython,
    "rust": serializer_rs,
}


@pytest.mark.parametrize("impl", ["cython", "rust"])
def test_dump(benchmark, impl):
    serializer = serializers[impl]
    serializer.dump(test_object)  # warmup

    benchmark.group = "dump"
    benchmark.extra_info["impl"] = impl
    benchmark.extra_info["correct"] = (
        serializer.load(serializer.dump(test_object)) == test_object
    )
    benchmark(serializer.dump, test_object)


@pytest.mark.parametrize("impl", ["cython", "rust"])
def test_load(benchmark, impl):
    serializer = serializers[impl]
    test_dict = serializer.dump(test_object)
    serializer.load(test_dict, validate=False)  # warmup

    benchmark.group = "load"
    benchmark.extra_info["impl"] = impl
    benchmark.extra_info["correct"] = (
        serializer.load(serializer.dump(test_object)) == test_object
    )
    benchmark(serializer.load, test_dict, validate=False)


@pytest.mark.parametrize("impl", ["rust", "cython"])
def test_load_validate(benchmark, impl):
    serializer = serializers[impl]
    test_dict = serializer.dump(test_object)
    serializer.load(test_dict, validate=True)  # warmup

    benchmark.group = "load with validate"
    benchmark.extra_info["impl"] = impl
    benchmark.extra_info["correct"] = (
        serializer.load(serializer.dump(test_object)) == test_object
    )

    benchmark(serializer.load, test_dict, validate=True)
