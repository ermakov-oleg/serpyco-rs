"""Behaviour that must hold whether or not an entity's fields are read and
written through their `__slots__` offsets.

`python::slots::resolve` enables the direct path only for a plain slots layout
and refuses everything else, so each case below pins one side of that decision:
the fast path must be indistinguishable from the descriptor path, and every
class shape that disables it must keep working exactly as before.
"""

from dataclasses import dataclass, field
from typing import Any, Optional, Union

import pytest

from serpyco_rs import Serializer, ValidationError

from ._codecs import as_python, parametrize_codec_or_dict


@dataclass(slots=True)
class Slotted:
    a: int
    b: str
    c: Optional[str] = None


@dataclass
class Plain:
    a: int
    b: str
    c: Optional[str] = None


@dataclass(slots=True, frozen=True)
class FrozenSlotted:
    a: int
    b: str


@dataclass(frozen=True)
class FrozenPlain:
    a: int
    b: str


@parametrize_codec_or_dict
@pytest.mark.parametrize('cls', [Slotted, Plain, FrozenSlotted, FrozenPlain])
def test_round_trip_for_every_class_shape(codec, cls):
    serializer = Serializer(cls, codec=codec)
    kwargs: dict[str, Any] = {'a': 1, 'b': 'x'}
    if cls in (Slotted, Plain):
        kwargs['c'] = 'y'
    obj = cls(**kwargs)
    assert as_python(codec, serializer.dump(obj)) == kwargs
    assert serializer.load(serializer.dump(obj)) == obj


@parametrize_codec_or_dict
def test_defaults_are_filled_for_absent_fields(codec):
    serializer = Serializer(Slotted, codec=codec)
    assert serializer.load(Serializer(dict[str, Any], codec=codec).dump({'a': 1, 'b': 'x'})) == Slotted(a=1, b='x')


@parametrize_codec_or_dict
def test_missing_required_field_still_reported(codec):
    serializer = Serializer(Slotted, codec=codec)
    raw = Serializer(dict[str, Any], codec=codec).dump({'a': 1})
    with pytest.raises(ValidationError):
        serializer.load(raw)


# --- shapes that must disable the direct path ------------------------------


@dataclass(slots=True)
class CustomSetattr:
    a: int

    def __setattr__(self, name: str, value: Any) -> None:
        # A slots class may still intercept assignment; a direct store would
        # silently skip this, so the optimization has to refuse the class.
        object.__setattr__(self, name, value * 2 if isinstance(value, int) else value)


def test_custom_setattr_is_still_called():
    serializer = Serializer(CustomSetattr)
    assert serializer.load({'a': 3}).a == 6


class SubclassOverridingAttribute(Slotted):
    """A subclass that shadows one of the base class's slots with a property."""

    __slots__ = ()

    @property  # type: ignore[misc]
    def b(self) -> str:
        return 'overridden'

    @b.setter
    def b(self, value: str) -> None:
        pass  # the dataclass __init__ still assigns it


@parametrize_codec_or_dict
def test_subclass_property_wins_over_the_base_slot(codec):
    # The encoder holds the base class's offsets, but a subclass instance must
    # be dumped through its own descriptors, not read at those offsets.
    serializer = Serializer(Slotted, codec=codec)
    obj = SubclassOverridingAttribute(a=1, b='ignored', c=None)
    assert as_python(codec, serializer.dump(obj))['b'] == 'overridden'


@dataclass
class HasDict:
    a: int


def test_class_with_instance_dict_round_trips():
    serializer = Serializer(HasDict)
    obj = serializer.load({'a': 1})
    assert obj == HasDict(a=1)
    assert serializer.dump(obj) == {'a': 1}


# --- dump-side edge cases --------------------------------------------------


@parametrize_codec_or_dict
def test_dump_rejects_an_object_of_the_wrong_shape(codec):
    # Untagged-union probing feeds arbitrary objects in on purpose; the encoder
    # must report a schema mismatch rather than read memory at a slot offset.
    serializer = Serializer(Slotted, codec=codec)
    with pytest.raises(ValidationError):
        serializer.dump(object())  # type: ignore[arg-type]


@dataclass(slots=True, repr=False)
class Unfilled:
    a: int
    b: str

    def __repr__(self) -> str:
        # The error path renders the offending value; the generated repr would
        # itself raise on an unassigned slot, which is noise for this test.
        return '<Unfilled>'


@parametrize_codec_or_dict
def test_dump_of_an_instance_with_an_unset_slot(codec):
    serializer = Serializer(Unfilled, codec=codec)
    obj = Unfilled.__new__(Unfilled)  # every slot is still empty
    with pytest.raises(ValidationError):
        serializer.dump(obj)


@parametrize_codec_or_dict
def test_union_of_slotted_entities_picks_the_right_member(codec):
    @dataclass(slots=True)
    class Left:
        left: int

    @dataclass(slots=True)
    class Right:
        right: str

    serializer = Serializer(list[Union[Left, Right]], codec=codec)
    values = [Left(left=1), Right(right='x')]
    assert as_python(codec, serializer.dump(values)) == [{'left': 1}, {'right': 'x'}]
    assert serializer.load(serializer.dump(values)) == values


@parametrize_codec_or_dict
def test_default_factory_and_mutation(codec):
    @dataclass(slots=True)
    class WithFactory:
        items: list[int] = field(default_factory=list)

    serializer = Serializer(WithFactory, codec=codec)
    first = serializer.load(Serializer(dict[str, Any], codec=codec).dump({}))
    second = serializer.load(Serializer(dict[str, Any], codec=codec).dump({}))
    first.items.append(1)
    assert second.items == []


@parametrize_codec_or_dict
def test_repeated_load_does_not_leak_the_previous_value(codec):
    # A direct slot store overwrites whatever was there; with a fresh instance
    # per load the slot is always empty, but the refcount must stay flat anyway.
    import gc
    import sys

    serializer = Serializer(Slotted, codec=codec)
    raw = serializer.dump(Slotted(a=1, b='x', c='y'))
    shared = 'a string held by the test'
    gc.collect()
    before = sys.getrefcount(shared)
    for _ in range(100):
        serializer.load(raw)
    gc.collect()
    assert sys.getrefcount(shared) == before
