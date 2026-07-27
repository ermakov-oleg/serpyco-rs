"""End-to-end ``bytes -> object -> bytes`` benchmarks.

``test_benchmarks.py`` measures the dict-oriented path (object to/from builtin
Python types); these measure the wire scenario, serializing straight to ``bytes``
and back. See ``CONTENDERS`` for what is compared.
"""

import msgspec
import orjson
import pytest

from .libs import msgspec_struct, serpyco_rs, serpyco_rs_codec


def _msgspec_dump(obj):
    return msgspec.json.encode(obj)


def _msgspec_load(data):
    return msgspec.json.decode(data, type=msgspec_struct.Dataclass)


# Each contender dumps/loads its own equivalently-shaped test object.
CONTENDERS = {
    'serpyco_rs_codec': {
        'dump': serpyco_rs_codec.dump,
        'load': serpyco_rs_codec.load,
        'obj': serpyco_rs_codec.test_object,
        'skip_refcount': False,
    },
    'serpyco_rs+orjson': {
        'dump': lambda o: orjson.dumps(serpyco_rs.dump(o)),
        'load': lambda b: serpyco_rs.load(orjson.loads(b)),
        'obj': serpyco_rs.test_object,
        # orjson's internal key cache shifts sys.gettotalrefcount across a
        # gc.collect() — orjson's behavior, not a leak here.
        'skip_refcount': True,
    },
    'msgspec': {
        'dump': _msgspec_dump,
        'load': _msgspec_load,
        'obj': msgspec_struct.test_object,
        'skip_refcount': True,
    },
}


@pytest.mark.parametrize('lib', CONTENDERS.keys())
def test_dump_to_bytes(bench_or_check_refcount, lib):
    contender = CONTENDERS[lib]
    dump, load, obj = contender['dump'], contender['load'], contender['obj']
    dump(obj)  # warmup

    bench_or_check_refcount.group = 'dump to bytes'
    bench_or_check_refcount.extra_info['lib'] = lib
    bench_or_check_refcount.extra_info['correct'] = load(dump(obj)) == obj
    if contender['skip_refcount']:
        bench_or_check_refcount.skip_refcount = True
    bench_or_check_refcount(dump, obj)


@pytest.mark.parametrize('lib', CONTENDERS.keys())
def test_load_from_bytes(bench_or_check_refcount, lib):
    contender = CONTENDERS[lib]
    dump, load, obj = contender['dump'], contender['load'], contender['obj']
    data = dump(obj)  # pre-encode the bytes once, then benchmark decode
    load(data)  # warmup

    bench_or_check_refcount.group = 'load from bytes'
    bench_or_check_refcount.extra_info['lib'] = lib
    bench_or_check_refcount.extra_info['correct'] = load(data) == obj
    if contender['skip_refcount']:
        bench_or_check_refcount.skip_refcount = True
    bench_or_check_refcount(load, data)
