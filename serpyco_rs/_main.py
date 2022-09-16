from __future__ import annotations

from typing import Any, TypeVar

from ._describe import describe_type
from ._impl import make_encoder, Serializer


T = TypeVar('T', bound=Any)


def make_serializer(t: type[T]) -> Serializer[T]:
    type_info = describe_type(t)
    return make_encoder(type_info)
