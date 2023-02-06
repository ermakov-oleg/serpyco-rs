import pkgutil

import pytest
from .libs import marshmallow, pydantic, serpyco, serpyco_rs, mashumaro

IS_PYTEST_CODSPEED_INSTALLED = pkgutil.find_loader("pytest_codspeed") is not None

serializers = {
    "serpyco_rs": serpyco_rs,
    **(
        {}
        if IS_PYTEST_CODSPEED_INSTALLED
        else {
            "serpyco": serpyco,
            "pydantic": pydantic,
            "marshmallow": marshmallow,
            "mashumaro": mashumaro,
        }
    ),
}


@pytest.mark.parametrize("lib", serializers.keys())
def test_dump(benchmark, lib):
    serializer = serializers[lib]
    serializer.dump(serializer.test_object)  # warmup

    benchmark.group = "dump"
    if not IS_PYTEST_CODSPEED_INSTALLED:
        benchmark.extra_info["lib"] = lib
        benchmark.extra_info["correct"] = (
            serializer.load(serializer.dump(serializer.test_object)) == serializer.test_object
        )
    benchmark(serializer.dump, serializer.test_object)


@pytest.mark.parametrize("lib", serializers.keys())
def test_load(benchmark, lib):
    serializer = serializers[lib]
    test_dict = serializer.dump(serializer.test_object)
    serializer.load(test_dict, validate=False)  # warmup

    benchmark.group = "load"
    if not IS_PYTEST_CODSPEED_INSTALLED:
        benchmark.extra_info["lib"] = lib
        benchmark.extra_info["correct"] = (
            serializer.load(serializer.dump(serializer.test_object)) == serializer.test_object
        )
    benchmark(serializer.load, test_dict, validate=False)


@pytest.mark.parametrize("lib", serializers.keys())
def test_load_validate(benchmark, lib):
    serializer = serializers[lib]
    test_dict = serializer.dump(serializer.test_object)
    serializer.load(test_dict, validate=True)  # warmup

    benchmark.group = "load with validate"
    if not IS_PYTEST_CODSPEED_INSTALLED:
        benchmark.extra_info["lib"] = lib
        benchmark.extra_info["correct"] = (
            serializer.load(serializer.dump(serializer.test_object)) == serializer.test_object
        )

    benchmark(serializer.load, test_dict, validate=True)
