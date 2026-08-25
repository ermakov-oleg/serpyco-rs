"""Correctness checks for the specialized GitHub-issue JSON codec (experiment).

Everything here compares against `serpyco_rs.Serializer(Issue, codec=JSON)` —
the prototype is only interesting if it is indistinguishable from it on this
model. Run with::

    .venv/bin/python -m pytest bench/experiments/specialized_json_codec -q
"""

import gc
import json
import sys
from pathlib import Path

import pytest
import serpyco_rs

from bench.compare.github_issue.serpyco_rs import (
    AuthorAssociation,
    Issue,
    IssueState,
    IssueStateReason,
)


sp = pytest.importorskip(
    'bench.experiments.specialized_json_codec.codec',
    reason='built without the `bench-codec` cargo feature',
)

DATA = (Path(__file__).parents[2] / 'compare/github_issue/data.json').read_bytes()
BASE = serpyco_rs.Serializer(Issue, codec=serpyco_rs.JSON)


def semantic(raw: bytes) -> dict:
    return json.loads(raw)


# --- 1. load parity ---------------------------------------------------------


def test_load_matches_serpyco():
    assert sp.load(DATA) == BASE.load(DATA)


def test_load_produces_the_benchmark_dataclasses():
    obj = sp.load(DATA)
    assert type(obj) is Issue
    assert obj.state is IssueState.OPEN
    assert obj.state_reason is IssueStateReason.REOPENED
    assert obj.author_association is AuthorAssociation.NONE
    assert type(obj.labels) is list
    assert obj.reactions.plus_one == 0 and obj.reactions.minus_one == 0
    assert obj.created_at.tzinfo is not None
    # Defaults that the document never mentions.
    assert obj.draft is False
    assert obj.body_html is None


# --- 2. dump parity ---------------------------------------------------------


def test_dump_matches_serpyco_semantically():
    obj = BASE.load(DATA)
    assert semantic(sp.dump(obj)) == semantic(BASE.dump(obj))


def test_dump_is_byte_identical_here():
    # Not a requirement (key order and separators are the codec's own choice),
    # but on this model both writers emit fields in declaration order with no
    # spaces, so any byte difference is worth looking at.
    obj = BASE.load(DATA)
    assert sp.dump(obj) == BASE.dump(obj)


# --- 3. round trip ----------------------------------------------------------


def test_round_trip():
    obj = sp.load(DATA)
    assert sp.load(sp.dump(obj)) == obj
    assert BASE.load(sp.dump(obj)) == obj
    assert sp.load(BASE.dump(obj)) == obj


# --- 4. key order -----------------------------------------------------------


def _shuffle_keys(node, rng):
    if isinstance(node, dict):
        items = [(k, _shuffle_keys(v, rng)) for k, v in node.items()]
        rng.shuffle(items)
        return dict(items)
    if isinstance(node, list):
        return [_shuffle_keys(v, rng) for v in node]
    return node


@pytest.mark.parametrize('seed', range(8))
def test_key_order_does_not_matter(seed):
    import random

    rng = random.Random(seed)
    shuffled = json.dumps(_shuffle_keys(json.loads(DATA), rng)).encode()
    assert sp.load(shuffled) == BASE.load(shuffled) == BASE.load(DATA)


def test_reversed_key_order():
    reversed_doc = json.dumps(dict(reversed(list(json.loads(DATA).items())))).encode()
    assert sp.load(reversed_doc) == BASE.load(DATA)


# --- 5. unknown fields ------------------------------------------------------


def test_unknown_fields_at_every_level():
    doc = json.loads(DATA)
    doc['unknown_root'] = {'a': [1, 2, {'b': None}], 'c': 'x'}
    doc['user']['unknown_user'] = [[[[1]]]]
    doc['labels'][0]['unknown_label'] = 'ignored'
    doc['labels'].append('a bare string label')
    doc['reactions']['unknown_reaction'] = 1.5
    doc['assignees'][0]['unknown_assignee'] = True
    raw = json.dumps(doc).encode()
    assert sp.load(raw) == BASE.load(raw)


def test_unknown_field_named_like_a_python_attribute():
    # `plus_one` is the *attribute*; the wire name is `+1`. A document carrying
    # the attribute name must not feed the field.
    doc = json.loads(DATA)
    doc['reactions']['plus_one'] = 999
    raw = json.dumps(doc).encode()
    assert sp.load(raw).reactions.plus_one == doc['reactions']['+1']
    assert sp.load(raw) == BASE.load(raw)


def test_aliases_round_trip():
    obj = sp.load(DATA)
    obj.reactions.plus_one = 7
    obj.reactions.minus_one = -3
    dumped = semantic(sp.dump(obj))
    assert dumped['reactions']['+1'] == 7
    assert dumped['reactions']['-1'] == -3
    assert dumped['reactions'] == semantic(BASE.dump(obj))['reactions']


# --- 6. escapes and unicode -------------------------------------------------

TRICKY = [
    'plain',
    '',
    'quote " backslash \\ slash /',
    'controls \b\f\n\r\t\x00\x1f',
    'unicode: привет 中文 🎉 é́',
    'surrogate pair via json: \U0001f600',
    'x' * 300 + ' ' + 'y' * 300,
    'mixed "ÿ" tail',
]


@pytest.mark.parametrize('text', TRICKY)
def test_escapes_and_unicode(text):
    doc = json.loads(DATA)
    doc['title'] = text
    doc['body'] = text
    doc['user']['name'] = text
    doc['labels'][0]['description'] = text
    raw = json.dumps(doc).encode()  # ensure_ascii=True -> \uXXXX escapes
    obj = sp.load(raw)
    assert obj == BASE.load(raw)
    assert obj.title == text
    raw_utf8 = json.dumps(doc, ensure_ascii=False).encode()  # raw UTF-8 bytes
    assert sp.load(raw_utf8) == BASE.load(raw_utf8)
    assert semantic(sp.dump(obj)) == semantic(BASE.dump(obj))


def test_escaped_key_is_treated_as_unknown():
    # A documented simplification: keys carrying escapes never match a field.
    raw = DATA.replace(b'"title":', b'"\\u0074itle":', 1)
    with pytest.raises(ValueError, match='missing required field'):
        sp.load(raw)
    assert BASE.load(raw).title  # the real codec still resolves it


# --- datetime ---------------------------------------------------------------


@pytest.mark.parametrize(
    'stamp',
    [
        '2023-04-04T05:56:47Z',
        '2023-04-04T05:56:47+00:00',
        '2023-04-04T05:56:47.123456Z',
        '2023-04-04T05:56:47.1Z',
        '2023-04-04T05:56:47.1234567Z',  # truncated to microseconds
        '2023-04-04T05:56:47-07:30',
        '2023-04-04T05:56:47+05:45',
        '2023-04-04t05:56:47Z',
        '2023-04-04 05:56:47Z',
        '2023-04-04T05:56:47',  # naive
    ],
)
def test_datetime_parity(stamp):
    doc = json.loads(DATA)
    doc['created_at'] = stamp
    raw = json.dumps(doc).encode()
    obj, ref = sp.load(raw), BASE.load(raw)
    assert obj.created_at == ref.created_at
    assert obj.created_at.tzinfo == ref.created_at.tzinfo
    assert semantic(sp.dump(obj))['created_at'] == semantic(BASE.dump(ref))['created_at']


# --- errors -----------------------------------------------------------------


def test_missing_required_field():
    doc = json.loads(DATA)
    del doc['title']
    with pytest.raises(ValueError, match='missing required field'):
        sp.load(json.dumps(doc).encode())


def test_bad_enum_value():
    doc = json.loads(DATA)
    doc['state'] = 'exploded'
    with pytest.raises(ValueError, match='invalid IssueState'):
        sp.load(json.dumps(doc).encode())


def test_truncated_document():
    with pytest.raises(ValueError):
        sp.load(DATA[: len(DATA) // 2])


def test_trailing_garbage():
    with pytest.raises(ValueError, match='trailing data'):
        sp.load(DATA + b'{}')


def test_dump_rejects_a_foreign_object():
    with pytest.raises(ValueError):
        sp.dump(object())


# --- ordered oracle ---------------------------------------------------------


def test_ordered_oracle_matches_and_is_layout_bound():
    sp.enable_ordered(DATA)
    assert sp.load_ordered(DATA) == BASE.load(DATA)
    reordered = json.dumps(dict(reversed(list(json.loads(DATA).items())))).encode()
    with pytest.raises(ValueError):
        sp.load_ordered(reordered)


# --- reference counting -----------------------------------------------------


def test_no_reference_leak_on_the_happy_path():
    obj = sp.load(DATA)
    watched = [Issue, IssueState.OPEN, AuthorAssociation.NONE, None, True, DATA]
    gc.collect()
    before = [sys.getrefcount(o) for o in watched]
    for _ in range(200):
        sp.dump(sp.load(DATA))
    gc.collect()
    after = [sys.getrefcount(o) for o in watched]
    assert before == after, list(zip(watched, before, after))
    del obj


def test_no_reference_leak_on_the_error_path():
    doc = json.loads(DATA)
    del doc['title']  # fails after several fields were already parsed
    raw = json.dumps(doc).encode()
    watched = [Issue, IssueState.OPEN, None, True]
    gc.collect()
    before = [sys.getrefcount(o) for o in watched]
    for _ in range(200):
        with pytest.raises(ValueError):
            sp.load(raw)
    gc.collect()
    after = [sys.getrefcount(o) for o in watched]
    assert before == after
