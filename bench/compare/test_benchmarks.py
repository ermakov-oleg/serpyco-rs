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
def test_dump(bench_or_check_refcount, lib):
    serializer = serializers[lib]
    serializer.dump(serializer.test_object)  # warmup

    bench_or_check_refcount.group = 'dump'
    bench_or_check_refcount.extra_info['lib'] = lib
    bench_or_check_refcount.extra_info['correct'] = (
        serializer.load(serializer.dump(serializer.test_object)) == serializer.test_object
    )
    if lib in {'serpyco', 'pydantic'}:
        bench_or_check_refcount.skip_refcount = True
    bench_or_check_refcount(serializer.dump, serializer.test_object)


@pytest.mark.parametrize('lib', serializers.keys())
def test_load_validate(bench_or_check_refcount, lib):
    serializer = serializers[lib]
    test_dict = serializer.dump(serializer.test_object)
    serializer.load(test_dict)  # warmup

    bench_or_check_refcount.group = 'load with validate'
    bench_or_check_refcount.extra_info['lib'] = lib
    bench_or_check_refcount.extra_info['correct'] = (
        serializer.load(serializer.dump(serializer.test_object)) == serializer.test_object
    )
    if lib in {'serpyco', 'pydantic'}:
        bench_or_check_refcount.skip_refcount = True
    bench_or_check_refcount(serializer.load, test_dict)
