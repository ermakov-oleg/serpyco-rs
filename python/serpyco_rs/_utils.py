import re

CAMELCASE_RE = re.compile(r"(?!^)_([a-zA-Z])")


def to_camelcase(s: str) -> str:
    return CAMELCASE_RE.sub(lambda m: m.group(1).upper(), s).rstrip("_")
