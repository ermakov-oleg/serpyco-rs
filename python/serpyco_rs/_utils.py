import re
from functools import lru_cache

from attributes_doc import get_attributes_doc as _get_attributes_doc


CAMELCASE_RE = re.compile(r'(?!^)_([a-zA-Z])')


def to_camelcase(s: str) -> str:
    return CAMELCASE_RE.sub(lambda m: m.group(1).upper(), s).rstrip('_')


get_attributes_doc = lru_cache(_get_attributes_doc)
