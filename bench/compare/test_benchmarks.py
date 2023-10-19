import json

import pytest

from .libs import marshmallow, mashumaro, pydantic, serpyco, serpyco_rs


serializers = {
    'serpyco_rs': serpyco_rs,
    'serpyco': serpyco,
    'pydantic': pydantic,
    'marshmallow': marshmallow,
    'mashumaro': mashumaro,
}


@pytest.mark.parametrize('lib', serializers.keys())
def test_dump(benchmark, lib):
    serializer = serializers[lib]
    serializer.dump(serializer.test_object)  # warmup

    benchmark.group = 'dump'
    benchmark.extra_info['lib'] = lib
    benchmark.extra_info['correct'] = serializer.load(serializer.dump(serializer.test_object)) == serializer.test_object
    benchmark(serializer.dump, serializer.test_object)


@pytest.mark.parametrize('lib', serializers.keys())
def test_load_validate(benchmark, lib):
    serializer = serializers[lib]
    test_dict = serializer.dump(serializer.test_object)
    serializer.load(test_dict)  # warmup

    benchmark.group = 'load with validate'
    benchmark.extra_info['lib'] = lib
    benchmark.extra_info['correct'] = serializer.load(serializer.dump(serializer.test_object)) == serializer.test_object
    benchmark(serializer.load, test_dict)
