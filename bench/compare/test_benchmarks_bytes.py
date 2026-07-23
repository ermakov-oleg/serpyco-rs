"""End-to-end ``bytes -> object -> bytes`` benchmarks.

Unlike ``test_benchmarks.py`` (which measures the dict-oriented path, i.e. an
object to/from builtin Python types), these benchmarks measure the honest wire
scenario: serialize straight to ``bytes`` and deserialize straight from
``bytes``. Three contenders are compared:

* ``serpyco_rs_codec`` -- the new codec path (``Serializer(T, codec=JSON)``),
  which encodes/decodes without materializing an intermediate ``dict``.
* ``serpyco_rs+orjson`` -- the classic path: serpyco-rs to/from a ``dict`` plus
  ``orjson`` for the JSON bytes layer.
* ``msgspec`` -- msgspec's native JSON bytes API (``msgspec.json.encode`` /
  ``msgspec.json.decode``), for context.
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
        'skip_refcount': False,
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
