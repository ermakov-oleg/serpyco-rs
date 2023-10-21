from typing import Any
from pathlib import Path
import orjson

import pytest

from .github_issue import mashumaro, serpyco_rs

serializers = {
    'serpyco_rs': serpyco_rs,
    'mashumaro': mashumaro,
}


@pytest.fixture(scope='module')
def data() -> dict[str, Any]:
    path = Path(__file__).parent / 'github_issue/data.json'
    return orjson.loads(path.read_text())


@pytest.mark.parametrize('lib', serializers.keys())
def test_dump_github_issue(benchmark, lib, data):
    serializer = serializers[lib]
    test_object = serializer.load(data)
    serializer.dump(test_object)  # warmup

    benchmark.group = 'dump github issue'
    benchmark.extra_info['lib'] = lib
    benchmark.extra_info['correct'] = serializer.load(serializer.dump(test_object)) == test_object
    benchmark(serializer.dump, test_object)


@pytest.mark.parametrize('lib', serializers.keys())
def test_load_github_issue(benchmark, lib, data):
    serializer = serializers[lib]
    test_object = serializer.load(data)  # warmup

    benchmark.group = 'load github issue'
    benchmark.extra_info['lib'] = lib
    benchmark.extra_info['correct'] = serializer.load(serializer.dump(test_object)) == test_object
    benchmark(serializer.load, data)
