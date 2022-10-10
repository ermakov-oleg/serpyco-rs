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
serializer_rs_rapid = serpyco_rs.Serializer(Dataclass, validator_cls=serpyco_rs.RapidJsonValidator)

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
    "rust_rapidjson": serializer_rs_rapid,
}


@pytest.mark.parametrize("impl", ["cython", "rust"])
def test_dump(benchmark, impl):
    serializer = serializers[impl]
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

    benchmark.group = "load"
    benchmark.extra_info["impl"] = impl
    benchmark.extra_info["correct"] = (
        serializer.load(serializer.dump(test_object)) == test_object
    )
    benchmark(serializer.load, test_dict, validate=False)


@pytest.mark.parametrize("impl", ["cython", "rust", "rust_rapidjson"])
def test_load_validate(benchmark, impl):
    serializer = serializers[impl]
    test_dict = serializer.dump(test_object)

    benchmark.group = "load with validate"
    benchmark.extra_info["impl"] = impl
    benchmark.extra_info["correct"] = (
        serializer.load(serializer.dump(test_object)) == test_object
    )

    benchmark(serializer.load, test_dict, validate=True)
