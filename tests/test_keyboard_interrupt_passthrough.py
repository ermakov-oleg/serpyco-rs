"""Regression: BaseException-only errors (KeyboardInterrupt, SystemExit) from a custom
encoder must propagate through Union without being swallowed."""

from typing import Annotated, Union

import pytest
from serpyco_rs import SchemaValidationError, Serializer
from serpyco_rs.metadata import CustomEncoder


def test_keyboard_interrupt_from_custom_decoder_propagates_through_union():
    def raise_keyboard_interrupt(value):
        raise KeyboardInterrupt()

    def identity(value):
        return value

    AnnotatedInt = Annotated[
        int,
        CustomEncoder[int, int](serialize=identity, deserialize=raise_keyboard_interrupt),
    ]

    serializer = Serializer(Union[AnnotatedInt, str])

    with pytest.raises(KeyboardInterrupt):
        serializer.load(42)


def test_value_error_from_custom_decoder_is_caught_by_union():
    """Counterpart: regular Exception subclasses MUST be swallowed by Union (retry)."""

    def raise_value_error(value):
        raise ValueError('nope')

    def identity(value):
        return value

    AnnotatedInt = Annotated[
        int,
        CustomEncoder[int, int](serialize=identity, deserialize=raise_value_error),
    ]

    serializer = Serializer(Union[AnnotatedInt, str])

    # int 42 makes the custom decoder raise ValueError → Union should fall through to str.
    # But 42 isn't str either, so it should ultimately raise SchemaValidationError, not ValueError.
    with pytest.raises(SchemaValidationError):
        serializer.load(42)
