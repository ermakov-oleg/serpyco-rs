"""Codec matrix shared by every codec-agnostic test module."""

from typing import Any, Optional

import pytest

from serpyco_rs import JSON, MSGPACK, Codec, Serializer


# One entry per supported byte format. Mirrors bench/test_codec_encoders.py's CODECS
# on purpose, so adding a format means updating one list, not two schemes.
CODECS = [JSON, MSGPACK]


def codec_id(codec: Optional[Codec]) -> str:
    return 'dict' if codec is None else codec._name


parametrize_codec = pytest.mark.parametrize('codec', CODECS, ids=codec_id)

# `None` is the dict path (`dump` returns Python objects, no bytes involved). Use this
# for behaviour that must hold identically whether or not a codec is in play, and pair
# it with `as_python` so a single assertion covers all three.
parametrize_codec_or_dict = pytest.mark.parametrize('codec', [None, *CODECS], ids=codec_id)


def dump_any(codec: Codec, value: Any) -> bytes:
    """Codec-agnostic stand-in for `json.dumps`: builds `load()` input without assuming JSON's syntax."""
    return Serializer(Any, codec=codec).dump(value)


def load_any(codec: Codec, raw: bytes) -> Any:
    """Codec-agnostic stand-in for `json.loads`: inspects a dump's shape without assuming JSON's syntax."""
    return Serializer(Any, codec=codec).load(raw)


def as_python(codec: Optional[Codec], dumped: Any) -> Any:
    """A dump's result as Python objects, so one assertion can cover the dict path and every codec."""
    return dumped if codec is None else load_any(codec, dumped)
