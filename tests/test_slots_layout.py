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
def test_repeated_load_does_not_leak_instances(codec):
    # Every instance holds a strong reference to its (heap) class, so the class's
    # refcount is an exact count of live instances — a load that leaked one, or
    # that failed part-way and left the object alive, shows up here.
    #
    # Deliberately not `sys.getrefcount(None)`: None is immortal from 3.12 on,
    # and before that its count drifts by a few from unrelated interpreter work.
    # The value-side leak is covered by the shared-default test below, which
    # watches an ordinary object.
    import gc
    import sys

    serializer = Serializer(Slotted, codec=codec)
    raw = serializer.dump(Slotted(a=1, b='x', c=None))
    serializer.load(raw)  # warm any lazily built state
    gc.collect()
    before = sys.getrefcount(Slotted)

    for _ in range(100):
        serializer.load(raw)
    gc.collect()

    assert sys.getrefcount(Slotted) == before, 'instances were not freed'


@parametrize_codec_or_dict
def test_repeated_load_does_not_leak_a_shared_default(codec):
    # The default is one shared object written into a slot on every load; it has
    # to be released again when the instance dies.
    import gc
    import sys

    sentinel = 'shared default value'

    @dataclass(slots=True)
    class WithDefault:
        a: int
        b: str = sentinel

    serializer = Serializer(WithDefault, codec=codec)
    raw = Serializer(dict[str, Any], codec=codec).dump({'a': 1})
    assert serializer.load(raw).b is sentinel
    gc.collect()
    before = sys.getrefcount(sentinel)

    for _ in range(100):
        serializer.load(raw)
    gc.collect()

    assert sys.getrefcount(sentinel) == before


@parametrize_codec_or_dict
def test_dump_does_not_leak(codec):
    import gc
    import sys

    serializer = Serializer(Slotted, codec=codec)
    value = 'a value living in a slot'
    obj = Slotted(a=1, b=value, c=None)
    serializer.dump(obj)
    gc.collect()
    before = sys.getrefcount(value)

    for _ in range(100):
        serializer.dump(obj)
    gc.collect()

    assert sys.getrefcount(value) == before


# --- the class is mutable after the serializer is built ---------------------


@parametrize_codec_or_dict
def test_field_replaced_by_a_property_after_construction(codec):
    # The verified offsets describe a layout the attribute protocol no longer
    # uses; the encoder has to notice and go back through the descriptor.
    @dataclass(slots=True)
    class Mutated:
        x: int

    serializer = Serializer(Mutated, codec=codec)
    obj = Mutated(x=1)
    Mutated.x = property(lambda self: 99)  # type: ignore[assignment]
    assert as_python(codec, serializer.dump(obj))['x'] == 99


@parametrize_codec_or_dict
def test_setattr_installed_after_construction(codec):
    @dataclass(slots=True)
    class Mutated:
        y: int

    serializer = Serializer(Mutated, codec=codec)

    def doubling_setattr(self, name, value):
        object.__setattr__(self, name, value * 2)

    Mutated.__setattr__ = doubling_setattr  # type: ignore[method-assign]
    raw = Serializer(dict[str, Any], codec=codec).dump({'y': 3})
    assert serializer.load(raw).y == 6


@parametrize_codec_or_dict
def test_unrelated_class_attribute_added_after_construction(codec):
    # Any change to the type invalidates its version tag, so this also drops to
    # the descriptor path — slower, but it must stay correct.
    @dataclass(slots=True)
    class Mutated:
        x: int

    serializer = Serializer(Mutated, codec=codec)
    Mutated.unrelated = 'added later'  # type: ignore[attr-defined]
    obj = Mutated(x=1)
    assert as_python(codec, serializer.dump(obj)) == {'x': 1}
    assert serializer.load(serializer.dump(obj)) == obj


@parametrize_codec_or_dict
def test_survives_the_one_off_mutation_copyreg_makes(codec):
    # The first pickle/deepcopy of a slots instance makes `copyreg` cache
    # `__slotnames__` in the class dict, which bumps the class version exactly
    # once. That must not retire the fast path for good, and above all must not
    # change what the serializer produces.
    import copy
    import pickle

    serializer = Serializer(Slotted, codec=codec)
    obj = Slotted(a=1, b='x', c='y')
    before = as_python(codec, serializer.dump(obj))

    copy.deepcopy(obj)
    pickle.loads(pickle.dumps(obj))

    assert as_python(codec, serializer.dump(obj)) == before
    assert serializer.load(serializer.dump(obj)) == obj


# --- user code can reach the instance while it is still being filled --------


@parametrize_codec_or_dict
def test_default_factory_can_see_the_half_built_instance(codec):
    # `default_factory` is user code running mid-load, and the instance is
    # GC-tracked by then, so it is reachable from `gc.get_objects()`. Whatever
    # the fast path does, the finished object must still be correct.
    import gc

    seen: list[Any] = []

    def factory() -> list[int]:
        seen.extend(o for o in gc.get_objects() if type(o).__name__ == 'Observed')
        return []

    @dataclass(slots=True)
    class Observed:
        a: int
        b: str
        items: list[int] = field(default_factory=factory)

    serializer = Serializer(Observed, codec=codec)
    raw = Serializer(dict[str, Any], codec=codec).dump({'a': 1, 'b': 'x'})
    obj = serializer.load(raw)
    assert obj == Observed(a=1, b='x', items=[])
    # `seen` can also hold instances left over from another parametrization of
    # this test; the claim is only that *this* one was reachable mid-load.
    assert any(o is obj for o in seen)
