"""Python glue for the specialized GitHub-issue JSON codec (experiment).

The Rust side lives in ``src/bench_codec`` and is compiled only under the
``bench-codec`` cargo feature::

    maturin develop --release --features bench-codec

It is deliberately *not* part of the public API: it is imported straight from
the extension module and is bound to the dataclasses in
``bench/compare/github_issue/serpyco_rs.py``.
"""

import json
from typing import Any

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


def _import_codec() -> Any:
    from serpyco_rs import _serpyco_rs

    try:
        return _serpyco_rs.GithubIssueCodec
    # ImportError, not RuntimeError: `pytest.importorskip` and the benchmark
    # runner both treat "the feature is not built" as "skip", not "fail".
    except AttributeError as exc:  # pragma: no cover - build guard
        raise ImportError(
            'serpyco_rs was built without the `bench-codec` feature; rebuild with\n'
            '  maturin develop --release --features bench-codec'
        ) from exc


def make_codec() -> Any:
    return _import_codec()(
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


codec = make_codec()


def load(data: bytes) -> Issue:
    return codec.load(data)


def dump(obj: Issue) -> bytes:
    return codec.dump(obj)


# --- ordered-fixture oracle -------------------------------------------------
#
# Records, per entity type, the exact key sequence of one priming document. The
# oracle loader then steps over each key by its known length without reading or
# comparing it. Every occurrence of an entity must agree, otherwise the plan is
# refused here rather than silently mis-decoding.

_ENTITY_OF_KEY_PARENT = {
    'user': 'User',
    'assignee': 'User',
    'closed_by': 'User',
    'creator': 'User',
}


def _collect(node: Any, entity: str, out: dict[str, list[str]]) -> None:
    keys = list(node.keys())
    previous = out.setdefault(entity, keys)
    if previous != keys:
        raise ValueError(f'{entity}: occurrences disagree on key order:\n  {previous}\n  {keys}')

    for key, value in node.items():
        if value is None:
            continue
        if key in _ENTITY_OF_KEY_PARENT:
            _collect(value, _ENTITY_OF_KEY_PARENT[key], out)
        elif key == 'assignees':
            for item in value:
                _collect(item, 'User', out)
        elif key == 'labels':
            for item in value:
                if isinstance(item, dict):
                    _collect(item, 'IssueLabel', out)
        elif key == 'milestone':
            _collect(value, 'Milestone', out)
        elif key == 'reactions':
            _collect(value, 'Reactions', out)


def enable_ordered(data: bytes) -> None:
    """Prime the oracle from ``data``. Setup-only; unfair by construction."""
    plans: dict[str, list[str]] = {}
    _collect(json.loads(data), 'Issue', plans)
    codec.enable_ordered(plans)


def load_ordered(data: bytes) -> Issue:
    return codec.load_ordered(data)


def scan_only(data: bytes) -> int:
    """Scan and syntax-check the document without building anything."""
    return codec.scan_only(data)
