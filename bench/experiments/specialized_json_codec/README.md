# Experiment: a fully specialized JSON codec for the GitHub-issue benchmark

**Question.** If the schema is known ahead of time and the whole path from raw
JSON bytes to Python dataclasses is one piece of specialized Rust, how low does
the latency of `bench/compare/github_issue` go? That number is the floor a
runtime specializer — a JIT over the encoder tree — could aim at.

**Constraints this prototype respects.** No jiter, neither directly nor through
`crate::format::Parser`. No serde_json, orjson or sonic-rs. No JSON AST, no Rust
`Value`, no intermediate Python `dict` anywhere in the hot path. `load` reads the
input `&[u8]` and produces finished objects; `dump` reads the dataclass and
appends bytes to one `Vec<u8>`. The output is the *ordinary* dataclasses from
`bench/compare/github_issue/serpyco_rs.py` — not a `msgspec.Struct`, not a
codec-specific type.

Everything is benchmark-only: the Rust lives behind the `bench-codec` cargo
feature, off by default, and never reaches a released wheel or the public API.

```bash
uv run maturin develop --release --features bench-codec
python -m pytest bench/experiments/specialized_json_codec -q
taskset -c 2 python -m bench.experiments.specialized_json_codec.run_bench
bench/experiments/specialized_json_codec/run_valgrind.sh
```

---

## Method

4-core x86-64, Linux 6.18, CPython 3.12.3. `--release` with the crate's existing
`lto = "thin"` / `codegen-units = 1`. Pinned to one CPU with `taskset -c 2`; GC
collected, frozen and disabled around every sample; 1 s warmup per contender;
**three separate processes × 11 interleaved rounds** each — every contender is
sampled once per round in the same order, so drift hits all of them equally. The
tables give the median of the three runs' medians, and the same for p10.

Payload: `bench/compare/github_issue/data.json`, 8489 bytes, pretty-printed.

Instruction counts come from `valgrind --tool=callgrind`. Each case runs twice at
different iteration counts and the two are subtracted, so interpreter startup
drops out. Both counts are taken past the warm-up ramp (1000 and 3000): CPython's
adaptive interpreter keeps shaving off a little for the first few hundred calls,
and a 100-vs-400 subtraction overstates the per-call cost by ~5 %.

---

## Results

### load: `bytes -> Issue`

| contender | median µs | p10 µs | saved | speedup | Ir/call |
|---|---:|---:|---:|---:|---:|
| `serpyco_rs.Serializer(Issue, codec=JSON)` — baseline | 18.59 | 18.19 | — | 1.00× | 205 630 |
| **specialized codec** | **8.46** | **8.30** | **−10.13 µs** | **2.20×** | **86 572** |
| specialized codec, ordered fixture (ORACLE) | 7.65 | 7.31 | −10.94 µs | 2.43× | 81 380 |
| msgspec → the same plain dataclasses¹ | 19.79 | 19.44 | +1.20 µs | 0.94× | — |
| msgspec → `msgspec.Struct` | 9.88 | 9.70 | −8.71 µs | 1.88× | 122 667 |
| orjson → `dict` (no schema at all) | 13.41 | 13.22 | −5.18 µs | 1.39× | — |
| hand-written Python ← already-parsed `dict` | 17.52 | 17.15 | −1.07 µs | 1.06× | — |
| *specialized scanner only, builds nothing* | *3.52* | *3.49* | | | *33 753* |
| *Python → Rust call floor* | *0.11* | *0.11* | | | |

¹ msgspec has no equivalent of serpyco's `Alias`, so `Reactions.+1` / `-1` cannot
be decoded into these dataclasses at all. That contender is fed a payload with
those two keys renamed to the attribute spelling; everything else is identical.

### dump: `Issue -> bytes`

| contender | median µs | p10 µs | saved | speedup | Ir/call |
|---|---:|---:|---:|---:|---:|
| `serpyco_rs.Serializer(Issue, codec=JSON)` — baseline | 8.47 | 8.34 | — | 1.00× | 94 760 |
| **specialized codec** | **3.12** | **3.05** | **−5.35 µs** | **2.71×** | **35 805** |
| msgspec ← the same plain dataclasses | 10.65 | 10.32 | +2.18 µs | 0.80× | — |
| msgspec ← `msgspec.Struct` | 5.14 | 5.04 | −3.33 µs | 1.65× | 55 258 |
| orjson ← `dict` (no schema at all) | 3.05 | 2.91 | −5.42 µs | 2.78× | — |

### round trip

| | baseline | specialized | saved | speedup |
|---|---:|---:|---:|---:|
| load + dump | 27.06 µs | 11.58 µs | **−15.48 µs** | **2.34×** |

### cold construction (one-off)

| | median | min |
|---|---:|---:|
| `serpyco_rs.Serializer(Issue, codec=JSON)` | 3993 µs | 3779 µs |
| `GithubIssueCodec(...)` | 142 µs | 115 µs |

Not a like-for-like comparison — the codec only resolves 85 slot offsets, 15 enum
members and one tzinfo, while the `Serializer` walks the whole `typing` model,
builds a schema, resolves generics and constructs an encoder tree. It is listed
because a JIT would pay its compile time *on top of* those 4 ms, and ~140 µs is
roughly what an offset-resolution + code-emission pass needs.

### the reference numbers, re-checked on this machine

| quoted | measured here | note |
|---|---|---|
| `load_bytes` ≈ 16.5 µs | **18.59 µs** | same build flags; this box is ~12 % slower |
| `dump_bytes` ≈ 6.6 µs | **8.47 µs** | likewise |
| jiter scan/validation only ≈ 6.1 µs | **≈ 6.8 µs** | not directly runnable (the prototype may not call jiter), but callgrind puts `StringDecoder::decode` + `object_step` + `object_key` at 36.8 % of the baseline load — consistent with the quoted figure |
| hard-coded dataclass load from a ready `dict` ≈ 5.4 µs | **17.52 µs** | as *Python*. 5.4 µs is not reachable from the interpreter: a hand-written builder that does no parsing at all is still 2.1× slower than the specialized codec including the parse. The quoted number must have been a Rust-side measurement |
| msgspec into the same plain dataclass ≈ 12.8 µs | **19.79 µs** | msgspec is fast into its own `Struct` (9.88 µs) and slower than serpyco into a plain dataclass, both directions |

---

## The ordered-fixture fast path is an unfair oracle

`codec.enable_ordered(document)` records, per entity type, the exact member
sequence of one document. `codec.load_ordered()` then walks that plan and steps
over each key **by its recorded length, without reading or comparing it**. Any
document whose layout differs is rejected.

That is not a decoder — it is knowledge about one payload compiled in. It is
listed separately and exists only to price the order-independent key dispatch:

```
specialized codec            8.46 µs   86 572 Ir
specialized ordered ORACLE   7.65 µs   81 380 Ir
                             ------
key scan + key match          0.81 µs   5 192 Ir
```

**0.8 µs out of 18.6.** Key dispatch is not where the money is — see the
conclusion.

---

## Where the remaining time goes

`load`, 8.46 µs, split by direct measurement rather than attribution:

| part | µs | how it was measured |
|---|---:|---|
| Python → Rust call | 0.11 | `scan_only(b"0")` |
| scanning + syntax validation | 3.41 | `scan_only(DATA)` minus the call floor |
| everything else | 4.94 | remainder — building the objects, field dispatch, freeing the previous graph |
| *of which key scan + match* | *0.81* | `load` minus `load_ordered` |

The 4.94 µs is close to a floor. One `Issue` is 105 `str`, ~40 `int`, 11 dataclass
instances, 3 `datetime` and 2 `list` — about 161 CPython allocations — and the
same loop tears the previous graph down. Callgrind agrees: after the codec's own
functions the top entries are `PyObject_Free` (8 900 Ir/call), `PyObject_Malloc`
(6 500), `PyUnicode_New` (5 900), `_Py_Dealloc` (4 000) and `memcpy` (2 700) —
about 32 % of the specialized load, in CPython's allocator, with nothing on the
Rust side able to move it.

`dump`, 3.12 µs, is string escaping and little else:

| function | Ir/call | share |
|---|---:|---:|
| `escape_into` | 19 103 | 53 % |
| `dump_user` (inlined key emission + `write_str`/`write_int`) | 3 863 | 11 % |
| `memcpy` (the clean runs escaping copies out) | 3 489 | 10 % |
| `dump_issue` | 3 250 | 9 % |
| `write_escaped_byte` | 1 133 | 3 % |
| `PyLong_AsLongLong` + `itoa` | 1 702 | 5 % |
| `write_datetime` | 581 | 2 % |

orjson dumping a schema-free `dict` lands at 3.05 µs, i.e. the specialized
encoder is already at "write these bytes out" speed. Anything further has to
attack escaping itself.

Both hot loops — find where a string ends, find the next byte needing an escape —
use SSE2. It is part of the x86-64 baseline, so unlike a `#[target_feature(avx2)]`
helper it inlines into its callers instead of forcing a call at every use site.
Replacing the SWAR versions with SSE2, and splitting the string scanner so the
clean-ASCII case inlines while escapes/UTF-8 stay `#[cold]`, was worth 1.4 µs on
load and 0.5 µs on dump by itself.

---

## What the baseline spends that the prototype does not

`callgrind_annotate` on `serpyco_rs.Serializer`, as a share of its 205 630 Ir
(load) and 94 760 Ir (dump) per call:

**load**

| | Ir/call | share |
|---|---:|---:|
| jiter `StringDecoder::decode` | 55 425 | 27.0 % |
| `load_object_streaming` | 16 755 | 8.1 % |
| jiter `object_step` | 12 650 | 6.2 % |
| `PyObject_GenericSetAttr` | 11 899 | 5.8 % |
| `StringEncoder::load_format` | 11 365 | 5.5 % |
| `core::slice::ascii::is_ascii` | 8 392 | 4.1 % |
| `_PyType_Lookup` | 7 652 | 3.7 % |
| jiter `object_key` | 7 544 | 3.7 % |
| `PyMember_SetOne` | 6 430 | 3.1 % |
| `set_attr_unchecked` | 4 667 | 2.3 % |
| `memcmp` + `hashbrown::get` (key routing) | 6 193 | 3.0 % |
| **the setattr chain, total** | **30 648** | **14.9 %** ≈ 2.8 µs |
| **jiter, total** | **75 619** | **36.8 %** ≈ 6.8 µs |

**dump**

| | Ir/call | share |
|---|---:|---:|
| `escape::escape_into` (SWAR) | 26 432 | 27.9 % |
| `_PyObject_GenericGetAttrWithDict` | 12 186 | 12.9 % |
| `EntityEncoder::dump_format` | 10 705 | 11.3 % |
| `_PyType_Lookup` | 7 161 | 7.6 % |
| `memcpy` | 6 340 | 6.7 % |
| `StringEncoder::dump_format` | 6 045 | 6.4 % |
| `PyObject_GetAttr` | 5 169 | 5.5 % |
| `PyMember_GetOne` | 3 340 | 3.5 % |
| **the getattr chain, total** | **28 913** | **30.5 %** ≈ 2.6 µs |

The five structural differences, in rough order of value:

1. **Attribute access.** The prototype resolves each `__slots__` member offset
   once and writes/reads instance memory directly. The baseline performs ~146
   attribute operations per `Issue` (31 + 3×21 + 6×7 + 10), each a
   `_PyType_Lookup` on the MRO cache plus a descriptor call: 2.8 µs on load,
   2.6 µs on dump.
2. **String decoding.** jiter validates UTF-8 and produces a `&str`, and
   `create_py_string` then re-scans the same bytes with `is_ascii` before
   building the object. The prototype's scan classifies the string as it looks
   for the closing quote, so the ASCII case reaches `PyUnicode_New` + `memcpy`
   with no second pass.
3. **No per-field indirection.** No `Box<dyn Encoder>` call per field, no `Field`
   lookup, no `FxHashMap<String, usize>` routing probe, no `SeenSet`. The key
   match is a `match` on byte-string literals and the field body is an arm the
   compiler sees through.
4. **Datetime.** The prototype parses RFC 3339 inline and reuses the cached
   `timezone.utc` singleton. The baseline runs speedate, then allocates a
   `PyDelta` and calls `PyTimeZone_FromOffset` for *every* timestamp; on dump it
   formats through a `String`.
5. **Enums and keys.** Enum load is a byte match into a fixed array, dump is
   pointer identity into pre-rendered `"value"` bytes, versus a `PyDict` lookup
   keyed on a freshly built Python `str`. Both writers pre-render keys, but the
   prototype emits `,"node_id":` as one literal `extend_from_slice` with no
   per-field loop or `omit_none` branch around it.

---

## Semantic differences from `serpyco_rs.Serializer(Issue, codec=JSON)`

The prototype is checked against the real codec on this model
(`test_specialized_codec.py`, 44 cases: load parity, dump parity, round trip,
shuffled and reversed key order, unknown keys at every level, escapes and
Unicode, aliases, ten datetime spellings, error paths, refcount stability on
both the happy and the error path). Inside that envelope the two are
interchangeable. Outside it:

**Errors**

1. Every failure is a plain `ValueError` with a short message. No
   `ValidationError` / `DecodeError`, no `ErrorItem`, no instance path. Syntax
   errors carry a byte offset; schema errors carry only a field name.

**Numbers**

2. Integers are `i64` only; the real codec promotes to arbitrary precision. An
   out-of-range token is an error here.
3. There is no float, `Decimal`, `UUID`, `bytes`, `dict`, `tuple`, `set`,
   `Literal` or `Any` reader — this model has none, so none was written.

**Strings and JSON syntax**

4. An object key containing an escape (`"title"`) never matches a field: it
   is treated as unknown and skipped. jiter unescapes keys, so the real codec
   resolves it. A test asserts this divergence rather than hiding it.
5. Unpaired surrogates in `\uXXXX` escapes become U+FFFD instead of being
   rejected.
6. Raw control bytes inside a string are accepted; strict JSON rejects them.
7. Any byte `<= 0x20` counts as inter-token whitespace, so a few malformed
   documents get further before failing. The token readers still reject them at
   the next real token.
8. Invalid UTF-8 surfaces as CPython's `UnicodeDecodeError`, not a `DecodeError`.

**Recursion**

9. Skipping an unknown value is capped at depth 64, hard-coded. There is no
   `max_recursion_depth`; the known structure has fixed depth.

**Datetime**

10. RFC 3339 only: `YYYY-MM-DD(T|t|space)HH:MM:SS[.frac][Z|z|±HH[:]MM]`.
    Fractional digits past six are truncated (matching speedate's configured
    behaviour). Range validation is delegated to CPython's `datetime`
    constructor, so out-of-range messages differ. Forms speedate accepts and
    this parser does not (unix timestamps, for one) are errors.
11. On dump, the value must be exactly `datetime.datetime`; the real codec
    accepts subclasses.

**Options and model features**

12. No serializer option is honoured: `omit_none`, `camelcase_fields`,
    `none_format`, `naive_datetime_to_utc`, `try_cast_from_string`,
    `pass_through_bytes`, custom `Format`, `Discriminator`, `Flatten`, generics
    and frozen dataclasses have no equivalent. The codec is bound to this one
    model with the defaults the benchmark uses.
13. The `IssueLabel | str` union is resolved by the lead byte alone. The real
    codec narrows the same way when only one member accepts the kind, so
    behaviour matches here — but a union of two object shapes would need its
    re-parse machinery, which does not exist.

**Layout assumptions**

14. Every entity must be a `@dataclass(slots=True)` whose fields are plain
    `member_descriptor`s. Offsets are read from the descriptor and then
    **verified against a probe instance** at construction, so a class that is not
    slotted, or whose layout does not match, is rejected there rather than
    corrupting memory later.
15. `__init__` / `__post_init__` are skipped — the real codec skips them too.
16. Enum members are matched by pointer identity on dump.

**Oracle**

17. `enable_ordered` / `load_ordered` assume the member layout of the priming
    document, as described above.

---

## Files

New Rust, all behind the `bench-codec` feature:

- [`src/bench_codec/mod.rs`](../../../src/bench_codec/mod.rs) — the `GithubIssueCodec` pyclass, slot layouts, enum tables, ordered plans
- [`src/bench_codec/scan.rs`](../../../src/bench_codec/scan.rs) — the JSON scanner
- [`src/bench_codec/simd.rs`](../../../src/bench_codec/simd.rs) — SSE2 byte-class search, shared by the scanner and the escaper
- [`src/bench_codec/load.rs`](../../../src/bench_codec/load.rs) — `bytes -> Issue`
- [`src/bench_codec/dump.rs`](../../../src/bench_codec/dump.rs) — `Issue -> bytes`
- [`src/bench_codec/slots.rs`](../../../src/bench_codec/slots.rs) — slot offset discovery and verification
- [`src/bench_codec/obj.rs`](../../../src/bench_codec/obj.rs) — owned-reference and pending-field helpers

Modified (the only two files outside the experiment):

- [`Cargo.toml`](../../../Cargo.toml) — adds the empty `bench-codec` feature
- [`src/lib.rs`](../../../src/lib.rs) — registers the class under `#[cfg(feature = "bench-codec")]`

New Python:

- [`codec.py`](codec.py) — binds the codec to the benchmark's dataclasses, builds ordered plans
- [`hardcoded_python.py`](hardcoded_python.py) — the hand-written `dict -> Issue` reference
- [`run_bench.py`](run_bench.py) — interleaved wall-clock runner, cold construction, callgrind driver mode
- [`run_valgrind.sh`](run_valgrind.sh) — deterministic `Ir` per call
- [`test_specialized_codec.py`](test_specialized_codec.py) — the 44 parity/robustness cases

`cargo fmt --all` is clean. `cargo clippy --release --features bench-codec
--all-targets -- -D warnings` reports nothing for `src/bench_codec`; it does
report 104 pre-existing `result_large_err` findings elsewhere in the crate with
the toolchain used here (rustc 1.94) — the count is byte-for-byte identical with
the feature off, so none of them come from this experiment.

---

## Is ~2× a realistic target for a runtime JIT?

**Yes — but only about a third of the win is the part a JIT is usually sold on.**

Splitting the 10.13 µs saved on load by what produced it, using the callgrind
shares above:

| where the 10.13 µs came from | ≈ µs | needs codegen? |
|---|---:|---|
| ~146 `setattr` calls replaced by direct slot writes | 2.8 | no — offsets can be resolved when the `Serializer` is built |
| string decoding: jiter validates UTF-8 into a `&str`, then `create_py_string` re-scans it for ASCII; the prototype classifies the string while it looks for the closing quote | 2.3 | no |
| datetime: inline RFC 3339 + the cached UTC singleton instead of speedate + `PyDelta` + `PyTimeZone_FromOffset` per timestamp | ~1.0 | no |
| enum members by byte match / pointer identity instead of `PyDict` lookups on a freshly built `str` | ~0.5 | no |
| **per-field machinery: the `Box<dyn Encoder>` call, the `FxHashMap` key routing probe, `SeenSet`, the instance-path push, per-encoder revalidation** | **~3.5** | **yes — this is the JIT's share** |

So the four schema specializations that need no code generation are worth about
1.6× on their own, and flattening the encoder tree adds the remaining ~1.25×.
A JIT that only did the flattening — leaving `setattr`, the double string scan
and speedate in place — would land well short of 2×.

Two more things a JIT design should take from these numbers.

**Key dispatch is not the bottleneck.** The oracle removes *all* key work — not
just the match, but reading the key bytes at all — and buys 0.81 µs out of 18.59.
Emitting a perfect-hash key dispatcher is the most obvious thing a JIT could do
and among the least valuable.

**The floor is close.** Of the specialized codec's 8.46 µs, ~4.9 µs is CPython
allocating and freeing the ~161 objects an `Issue` is made of, and 0.11 µs is the
FFI boundary. That is a hard ceiling near **4–5×** on load for *any*
implementation that must return these dataclasses, and this prototype already
covered about half the distance to it. For dump the ceiling is nearer still:
3.12 µs, against orjson's 3.05 µs for a schema-free `dict`, with 53 % of what is
left sitting in string escaping.

Practical reading: **the first ~1.6× on load and ~2.4× on dump look available
without any JIT**, by specializing four specific things inside the existing
encoder tree. A JIT is how you would collect the rest, and it would have to earn
its compile time against a `Serializer` construction that already costs 4 ms.
