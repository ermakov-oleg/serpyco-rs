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
def test_dump_github_issue(bench_or_check_refcount, lib, data):
    serializer = serializers[lib]
    test_object = serializer.load(data)
    serializer.dump(test_object)  # warmup

    bench_or_check_refcount.group = 'dump github issue'
    bench_or_check_refcount.extra_info['lib'] = lib
    bench_or_check_refcount.extra_info['correct'] = serializer.load(serializer.dump(test_object)) == test_object
    bench_or_check_refcount(serializer.dump, test_object)


@pytest.mark.parametrize('lib', serializers.keys())
def test_load_github_issue(bench_or_check_refcount, lib, data):
    serializer = serializers[lib]
    test_object = serializer.load(data)  # warmup

    bench_or_check_refcount.group = 'load github issue'
    bench_or_check_refcount.extra_info['lib'] = lib
    bench_or_check_refcount.extra_info['correct'] = serializer.load(serializer.dump(test_object)) == test_object
    bench_or_check_refcount(serializer.load, data)
