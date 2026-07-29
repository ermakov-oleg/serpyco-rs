import json
from pathlib import Path

import pytest


LIBS = (
    'serpyco_rs_codec',
    'serpyco_rs+orjson',
    'msgspec',
    'mashumaro+orjson',
    'serpyco_rs_codec_msgpack',
    'serpyco_rs+ormsgpack',
    'msgspec_msgpack',
    'mashumaro+ormsgpack',
)


# Resolved per test rather than at import time: the PGO profile run selects the
# codec contenders only, and it runs in the build matrix (3.10 … 3.14t) where the
# comparison libs are not installed.
#
# ``wire`` converts the JSON fixture into the contender's native byte form
# (identity for JSON contenders, a one-time re-encode for MessagePack ones).
def _contender(lib: str) -> dict:
    if lib == 'serpyco_rs_codec':
        from .github_issue import serpyco_rs_codec

        return {'dump': serpyco_rs_codec.dump, 'load': serpyco_rs_codec.load, 'skip_refcount': False}

    if lib == 'serpyco_rs_codec_msgpack':
        from .github_issue import serpyco_rs_codec_msgpack

        return {
            'dump': serpyco_rs_codec_msgpack.dump,
            'load': serpyco_rs_codec_msgpack.load,
            'wire': serpyco_rs_codec_msgpack.from_json,
            'skip_refcount': False,
        }

    if lib == 'serpyco_rs+orjson':
        import orjson

        from .github_issue import serpyco_rs

        return {
            'dump': lambda o: orjson.dumps(serpyco_rs.dump(o)),
            'load': lambda b: serpyco_rs.load(orjson.loads(b)),
            # orjson's internal key cache shifts sys.gettotalrefcount across a
            # gc.collect() — orjson's behavior, not a leak here.
            'skip_refcount': True,
        }

    if lib == 'serpyco_rs+ormsgpack':
        import ormsgpack

        from .github_issue import serpyco_rs

        return {
            'dump': lambda o: ormsgpack.packb(serpyco_rs.dump(o)),
            'load': lambda b: serpyco_rs.load(ormsgpack.unpackb(b)),
            'wire': lambda data: ormsgpack.packb(json.loads(data)),
            # Same key-cache behavior as orjson (ormsgpack is an orjson fork).
            'skip_refcount': True,
        }

    if lib == 'msgspec':
        # Aliased so the sibling module named ``msgspec`` never shadows the package.
        from .github_issue import msgspec as msgspec_issue

        return {'dump': msgspec_issue.dump, 'load': msgspec_issue.load, 'skip_refcount': True}

    if lib == 'msgspec_msgpack':
        from .github_issue import msgspec_msgpack

        return {
            'dump': msgspec_msgpack.dump,
            'load': msgspec_msgpack.load,
            'wire': msgspec_msgpack.from_json,
            'skip_refcount': True,
        }

    if lib == 'mashumaro+ormsgpack':
        import ormsgpack

        from .github_issue import mashumaro

        return {
            'dump': lambda o: ormsgpack.packb(mashumaro.dump(o)),
            'load': lambda b: mashumaro.load(ormsgpack.unpackb(b)),
            'wire': lambda data: ormsgpack.packb(json.loads(data)),
            'skip_refcount': True,
        }

    import orjson

    from .github_issue import mashumaro

    return {
        'dump': lambda o: orjson.dumps(mashumaro.dump(o)),
        'load': lambda b: mashumaro.load(orjson.loads(b)),
        'skip_refcount': True,
    }


@pytest.fixture(scope='module')
def data() -> bytes:
    return (Path(__file__).parent / 'github_issue/data.json').read_bytes()


@pytest.mark.parametrize('lib', LIBS)
def test_dump_to_bytes(bench_or_check_refcount, lib, data):
    contender = _contender(lib)
    dump, load = contender['dump'], contender['load']
    wire = contender.get('wire', lambda b: b)(data)
    obj = load(wire)
    dump(obj)  # warmup

    bench_or_check_refcount.group = 'dump github issue to bytes'
    bench_or_check_refcount.extra_info['lib'] = lib
    bench_or_check_refcount.extra_info['correct'] = load(dump(obj)) == obj
    if contender['skip_refcount']:
        bench_or_check_refcount.skip_refcount = True
    bench_or_check_refcount(dump, obj)


@pytest.mark.parametrize('lib', LIBS)
def test_load_from_bytes(bench_or_check_refcount, lib, data):
    contender = _contender(lib)
    dump, load = contender['dump'], contender['load']
    wire = contender.get('wire', lambda b: b)(data)
    obj = load(wire)  # warmup + reference object
    load(wire)  # warmup

    bench_or_check_refcount.group = 'load github issue from bytes'
    bench_or_check_refcount.extra_info['lib'] = lib
    bench_or_check_refcount.extra_info['correct'] = load(dump(obj)) == obj
    if contender['skip_refcount']:
        bench_or_check_refcount.skip_refcount = True
    bench_or_check_refcount(load, wire)
