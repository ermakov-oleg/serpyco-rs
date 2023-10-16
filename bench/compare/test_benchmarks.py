import json

import pytest

from .libs import marshmallow, mashumaro, pydantic, serpyco, serpyco_rs


serializers = {
    'serpyco_rs': serpyco_rs,
    'serpyco': serpyco,
    'pydantic': pydantic,
    # 'marshmallow': marshmallow,
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
def test_dump_1(lib):
    serializer = serializers[lib]
    serializer.dump(serializer.test_object)  # warmup

    for _i in range(10000):
        serializer.dump(serializer.test_object)


@pytest.mark.parametrize('lib', serializers.keys())
def test_load_1(lib):
    serializer = serializers[lib]
    test_dict = json.dumps(serializer.dump(serializer.test_object))
    # serializer.load(test_dict, validate=True)  # warmup

    for _i in range(100):
        serializer.load_json(test_dict, validate=True)


@pytest.mark.parametrize('lib', serializers.keys())
def test_load(benchmark, lib):
    serializer = serializers[lib]
    test_dict = serializer.dump(serializer.test_object)
    serializer.load(test_dict, validate=False)  # warmup

    benchmark.group = 'load'
    benchmark.extra_info['lib'] = lib
    benchmark.extra_info['correct'] = serializer.load(serializer.dump(serializer.test_object)) == serializer.test_object
    benchmark(serializer.load, test_dict, validate=False)


@pytest.mark.parametrize('lib', serializers.keys())
def test_load_validate(benchmark, lib):
    serializer = serializers[lib]
    test_dict = serializer.dump(serializer.test_object)
    serializer.load(test_dict, validate=True)  # warmup

    benchmark.group = 'load with validate'
    benchmark.extra_info['lib'] = lib
    benchmark.extra_info['correct'] = serializer.load(serializer.dump(serializer.test_object)) == serializer.test_object
    benchmark(serializer.load, test_dict, validate=True)


@pytest.mark.parametrize('lib', serializers.keys())
def test_load_json(benchmark, lib):
    serializer = serializers[lib]
    test_data = json.dumps(serializer.dump(serializer.test_object))
    serializer.load_json(test_data, validate=False)  # warmup

    benchmark.group = 'load raw json'
    benchmark.extra_info['lib'] = lib
    benchmark.extra_info['correct'] = serializer.load_json(test_data) == serializer.test_object
    benchmark(serializer.load_json, test_data, validate=False)


@pytest.mark.parametrize('lib', serializers.keys())
def test_load_json_validate(benchmark, lib):
    serializer = serializers[lib]
    test_data = json.dumps(serializer.dump(serializer.test_object))
    serializer.load_json(test_data, validate=True)  # warmup

    benchmark.group = 'load raw json with validate'
    benchmark.extra_info['lib'] = lib
    benchmark.extra_info['correct'] = serializer.load_json(test_data) == serializer.test_object
    benchmark(serializer.load_json, test_data, validate=True)
