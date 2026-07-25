"""End-to-end ``bytes -> object -> bytes`` benchmarks on the github-issue payload.

The companion of ``test_github_issue.py`` (the dict-oriented path): these
benchmarks measure the honest wire scenario -- decode straight from the raw
``data.json`` bytes and encode straight back to ``bytes`` -- on serpyco-rs's real
"github issue" model. Four contenders are compared:

* ``serpyco_rs_codec`` -- the codec path (``Serializer(Issue, codec=JSON)``),
  which encodes/decodes without materializing an intermediate ``dict``.
* ``serpyco_rs+orjson`` -- the classic path: serpyco-rs to/from a ``dict`` plus
  ``orjson`` for the JSON bytes layer.
* ``msgspec`` -- msgspec's native JSON bytes API.
* ``mashumaro+orjson`` -- mashumaro to/from a ``dict`` plus ``orjson``.
"""

from pathlib import Path

import pytest


LIBS = ('serpyco_rs_codec', 'serpyco_rs+orjson', 'msgspec', 'mashumaro+orjson')


# Resolved per test rather than at import time: the PGO profile run selects the
# codec contender only, and it runs in the build matrix (3.10 … 3.14t) where the
# comparison libs are not installed.
def _contender(lib: str) -> dict:
    if lib == 'serpyco_rs_codec':
        from .github_issue import serpyco_rs_codec

        return {'dump': serpyco_rs_codec.dump, 'load': serpyco_rs_codec.load, 'skip_refcount': False}

    if lib == 'serpyco_rs+orjson':
        import orjson

        from .github_issue import serpyco_rs

        return {
            'dump': lambda o: orjson.dumps(serpyco_rs.dump(o)),
            'load': lambda b: serpyco_rs.load(orjson.loads(b)),
            # orjson keeps an internal key cache whose entries shift
            # sys.gettotalrefcount across a gc.collect(); that is orjson's behavior,
            # not a leak in this project (the serpyco_rs_codec contender is the one
            # whose refcounts we actually guard).
            'skip_refcount': True,
        }

    if lib == 'msgspec':
        # The sibling module is literally named ``msgspec``; alias it so it never
        # shadows the top-level ``msgspec`` package.
        from .github_issue import msgspec as msgspec_issue

        return {'dump': msgspec_issue.dump, 'load': msgspec_issue.load, 'skip_refcount': True}

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
    obj = load(data)
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
    obj = load(data)  # warmup + reference object
    load(data)  # warmup

    bench_or_check_refcount.group = 'load github issue from bytes'
    bench_or_check_refcount.extra_info['lib'] = lib
    bench_or_check_refcount.extra_info['correct'] = load(dump(obj)) == obj
    if contender['skip_refcount']:
        bench_or_check_refcount.skip_refcount = True
    bench_or_check_refcount(load, data)
