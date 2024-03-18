import sys
from typing import Any


def get_dataclass_args() -> dict[str, Any]:
    # python 3.9 does not support dataclasses(slot=True)
    if sys.version_info < (3, 10):
        return {}
    return {"slots": True}
