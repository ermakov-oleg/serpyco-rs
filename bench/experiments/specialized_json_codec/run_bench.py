"""Benchmark runner for the specialized GitHub-issue JSON codec (experiment).

Wall-clock mode (default) runs every contender in interleaved rounds so that
thermal drift and scheduler noise hit all of them equally, then reports the
median and the p10 (best decile) of the per-round means.

Instruction-count mode (`--callgrind`) runs a single contender in a tight loop
so the run can be wrapped in `valgrind --tool=callgrind`; see `run_valgrind.sh`.

    taskset -c 2 python -m bench.experiments.specialized_json_codec.run_bench
"""

from __future__ import annotations

import argparse
import gc
import json
import os
import statistics
import sys
import time
from collections.abc import Callable
from pathlib import Path


DATA = (Path(__file__).parents[2] / 'compare/github_issue/data.json').read_bytes()


def build_contenders() -> tuple[dict[str, Callable[[], object]], dict[str, Callable[[], object]]]:
    """Returns ``(load_cases, dump_cases)`` as zero-argument thunks."""
    import serpyco_rs

    from bench.compare.github_issue import msgspec as msgspec_struct
    from bench.compare.github_issue.serpyco_rs import Issue
    from bench.experiments.specialized_json_codec import codec as sp

    base = serpyco_rs.Serializer(Issue, codec=serpyco_rs.JSON)
    sp.enable_ordered(DATA)

    import msgspec

    # msgspec has no equivalent of serpyco's `Alias`, so the two Reactions keys
    # spelled `+1`/`-1` on the wire cannot be decoded into the same dataclass.
    # This contender therefore reads a payload with those two keys renamed to
    # the attribute spelling; everything else is identical, so it still prices
    # "msgspec filling the benchmark's plain dataclasses".
    msgspec_dc = msgspec.json.Decoder(Issue)
    msgspec_dc_enc = msgspec.json.Encoder()
    renamed = json.loads(DATA)
    renamed['reactions']['plus_one'] = renamed['reactions'].pop('+1')
    renamed['reactions']['minus_one'] = renamed['reactions'].pop('-1')
    msgspec_dc_payload = json.dumps(renamed).encode()

    obj = base.load(DATA)
    sp_obj = sp.load(DATA)
    ms_obj = msgspec_struct.load(DATA)
    ms_dc_obj = msgspec_dc.decode(msgspec_dc_payload)

    load_cases = {
        'serpyco-rs JSON codec (baseline)': lambda: base.load(DATA),
        'specialized codec': lambda: sp.load(DATA),
        'specialized codec, ordered (ORACLE)': lambda: sp.load_ordered(DATA),
        'msgspec -> dataclass (aliases renamed)': lambda: msgspec_dc.decode(msgspec_dc_payload),
        'msgspec -> msgspec.Struct': lambda: msgspec_struct.load(DATA),
        'specialized scanner only (no objects)': lambda: sp.scan_only(DATA),
        'python -> rust call floor': lambda: sp.scan_only(b'0'),
    }

    # Reference point (not a codec): hand-written Python, fed an already-parsed
    # dict, so it pays only for walking the schema and building the objects.
    from bench.experiments.specialized_json_codec import hardcoded_python

    parsed = json.loads(DATA)
    assert hardcoded_python.load(parsed) == obj
    load_cases['hand-written Python <- ready dict (no parsing)'] = lambda: hardcoded_python.load(parsed)
    dump_cases = {
        'serpyco-rs JSON codec (baseline)': lambda: base.dump(obj),
        'specialized codec': lambda: sp.dump(sp_obj),
        'msgspec <- dataclass': lambda: msgspec_dc_enc.encode(ms_dc_obj),
        'msgspec <- msgspec.Struct': lambda: msgspec_struct.dump(ms_obj),
    }

    try:
        import orjson

        load_cases['orjson.loads -> dict (no schema)'] = lambda: orjson.loads(DATA)
        plain = json.loads(DATA)
        dump_cases['orjson.dumps <- dict (no schema)'] = lambda: orjson.dumps(plain)
    except ImportError:
        pass

    return load_cases, dump_cases


# --- timing -----------------------------------------------------------------


def calibrate(fn: Callable[[], object], target_ns: int = 20_000_000) -> int:
    """Iterations that take roughly `target_ns` (default 20 ms) per sample."""
    n = 16
    while True:
        start = time.perf_counter_ns()
        for _ in range(n):
            fn()
        elapsed = time.perf_counter_ns() - start
        if elapsed >= target_ns // 4 or n >= 1 << 20:
            return max(8, int(n * target_ns / max(elapsed, 1)))
        n *= 4


def sample(fn: Callable[[], object], n: int) -> float:
    """Mean per-call microseconds over `n` calls, with the GC off."""
    gc.collect()
    gc.freeze()
    gc.disable()
    try:
        start = time.perf_counter_ns()
        for _ in range(n):
            fn()
        elapsed = time.perf_counter_ns() - start
    finally:
        gc.enable()
        gc.unfreeze()
    return elapsed / n / 1000


def measure(cases: dict[str, Callable[[], object]], rounds: int, warmup_s: float) -> dict[str, dict]:
    counts = {}
    for name, fn in cases.items():
        deadline = time.perf_counter() + warmup_s
        while time.perf_counter() < deadline:
            for _ in range(64):
                fn()
        counts[name] = calibrate(fn)

    samples: dict[str, list[float]] = {name: [] for name in cases}
    for _ in range(rounds):
        # Interleaved: one sample per contender per round, same order every time.
        for name, fn in cases.items():
            samples[name].append(sample(fn, counts[name]))

    return {
        name: {
            'median': statistics.median(values),
            'p10': sorted(values)[max(0, round(0.1 * (len(values) - 1)))],
            'min': min(values),
            'iters': counts[name],
        }
        for name, values in samples.items()
    }


def report(title: str, results: dict[str, dict], baseline: str) -> None:
    ref = results[baseline]['median']
    ref_p10 = results[baseline]['p10']
    width = max(len(n) for n in results)
    print(f'\n{title}')
    print(f'{"contender".ljust(width)}  {"median us":>10}  {"p10 us":>8}  {"delta us":>9}  {"speedup":>8}')
    print('-' * (width + 42))
    for name, r in results.items():
        print(
            f'{name.ljust(width)}  {r["median"]:10.2f}  {r["p10"]:8.2f}  '
            f'{ref - r["median"]:+9.2f}  {ref / r["median"]:7.2f}x'
        )
    print(f'(baseline p10 {ref_p10:.2f} us; {results[baseline]["iters"]} iterations per sample)')


# --- cold construction ------------------------------------------------------


def measure_construction(repeats: int = 20) -> None:
    import serpyco_rs

    from bench.compare.github_issue.serpyco_rs import (
        AuthorAssociation,
        Issue,
        IssueLabel,
        IssueState,
        IssueStateReason,
        Milestone,
        MilestoneState,
        Reactions,
        User,
    )
    from bench.experiments.specialized_json_codec.codec import _import_codec

    codec_cls = _import_codec()
    args = (
        Issue,
        User,
        IssueLabel,
        Milestone,
        Reactions,
        IssueState,
        MilestoneState,
        IssueStateReason,
        AuthorAssociation,
    )

    cases = {
        'serpyco_rs.Serializer(Issue, codec=JSON)': lambda: serpyco_rs.Serializer(Issue, codec=serpyco_rs.JSON),
        'GithubIssueCodec(...)': lambda: codec_cls(*args),
    }
    print('\ncold construction (one-off, per call)')
    width = max(len(n) for n in cases)
    for name, fn in cases.items():
        times = []
        for _ in range(repeats):
            gc.collect()
            start = time.perf_counter_ns()
            fn()
            times.append((time.perf_counter_ns() - start) / 1000)
        print(f'{name.ljust(width)}  median {statistics.median(times):9.1f} us  min {min(times):9.1f} us')


# --- callgrind mode ---------------------------------------------------------


def callgrind_mode(which: str, iterations: int) -> None:
    load_cases, dump_cases = build_contenders()
    cases = {**{f'load:{k}': v for k, v in load_cases.items()}, **{f'dump:{k}': v for k, v in dump_cases.items()}}
    fn = cases[which]
    for _ in range(200):  # warm caches; subtracted by the empty-loop control run
        fn()
    gc.collect()
    gc.disable()
    for _ in range(iterations):
        fn()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument('--rounds', type=int, default=9, help='interleaved measurement rounds')
    parser.add_argument('--warmup', type=float, default=0.5, help='warmup seconds per contender')
    parser.add_argument('--callgrind', metavar='CASE', help='run one case in a tight loop')
    parser.add_argument('--iterations', type=int, default=200)
    parser.add_argument('--list', action='store_true')
    args = parser.parse_args()

    if hasattr(os, 'sched_setaffinity') and not os.environ.get('SKIP_AFFINITY'):
        try:
            os.sched_setaffinity(0, {sorted(os.sched_getaffinity(0))[-1]})
        except OSError:
            pass

    if args.list:
        load_cases, dump_cases = build_contenders()
        for k in load_cases:
            print(f'load:{k}')
        for k in dump_cases:
            print(f'dump:{k}')
        return

    if args.callgrind:
        callgrind_mode(args.callgrind, args.iterations)
        return

    load_cases, dump_cases = build_contenders()
    print(
        f'python {sys.version.split()[0]}  cpu {sorted(os.sched_getaffinity(0))}  '
        f'rounds {args.rounds}  payload {len(DATA)} bytes'
    )
    report('load: bytes -> Issue', measure(load_cases, args.rounds, args.warmup), 'serpyco-rs JSON codec (baseline)')
    report('dump: Issue -> bytes', measure(dump_cases, args.rounds, args.warmup), 'serpyco-rs JSON codec (baseline)')
    measure_construction()


if __name__ == '__main__':
    main()
