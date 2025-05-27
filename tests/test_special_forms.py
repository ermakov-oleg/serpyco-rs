from typing import Annotated, ClassVar, Final, NewType, Union

from serpyco_rs._meta import Annotations
from serpyco_rs._secial_forms import unwrap_special_forms
from serpyco_rs.metadata import Alias, Max, Min
from typing_extensions import NotRequired, ReadOnly, Required


def test_bare_type():
    assert unwrap_special_forms(int) == (int, Annotations())


def test_new_type():
    Foo = NewType('Foo', int)
    assert unwrap_special_forms(Foo) == (int, Annotations())


def test_required():
    Foo = Required[int]
    assert unwrap_special_forms(Foo) == (int, Annotations())


def test_not_required():
    Foo = NotRequired[int]
    assert unwrap_special_forms(Foo) == (int, Annotations())


def test_class_var():
    Foo = ClassVar[int]
    assert unwrap_special_forms(Foo) == (int, Annotations())


def test_final():
    Foo = Final[int]
    assert unwrap_special_forms(Foo) == (int, Annotations())


def test_readonly():
    Foo = ReadOnly[int]
    assert unwrap_special_forms(Foo) == (int, Annotations())


def test_annotated_simple():
    Foo = Annotated[int, Min(5)]
    result_type, result_metadata = unwrap_special_forms(Foo)
    assert result_type == int
    assert result_metadata.get(Min) == Min(5)


def test_annotated_multiple_metadata():
    Foo = Annotated[str, Min(5), Max(10), Alias('foo')]
    result_type, result_metadata = unwrap_special_forms(Foo)
    assert result_type == str
    assert result_metadata.get(Min) == Min(5)
    assert result_metadata.get(Max) == Max(10)
    assert result_metadata.get(Alias) == Alias('foo')


def test_nested_annotated():
    Inner = Annotated[int, Min(5)]
    Outer = Annotated[Inner, Max(10)]
    result_type, result_metadata = unwrap_special_forms(Outer)
    assert result_type == int
    assert result_metadata.get(Min) == Min(5)
    assert result_metadata.get(Max) == Max(10)


def test_annotated_with_newtype():
    UserId = NewType('UserId', int)
    Foo = Annotated[UserId, Min(1)]
    result_type, result_metadata = unwrap_special_forms(Foo)
    assert result_type == int
    assert result_metadata.get(Min) == Min(1)


def test_annotated_with_required():
    Foo = Annotated[Required[int], Min(5)]
    result_type, result_metadata = unwrap_special_forms(Foo)
    assert result_type == int
    assert result_metadata.get(Min) == Min(5)


def test_complex_nested_forms():
    UserId = NewType('UserId', int)
    AnnotatedUserId = Annotated[UserId, Min(1)]
    RequiredAnnotatedUserId = Required[AnnotatedUserId]

    result_type, result_metadata = unwrap_special_forms(RequiredAnnotatedUserId)
    assert result_type == int
    assert result_metadata.get(Min) == Min(1)


def test_generic_types_not_unwrapped():
    result_type, result_metadata = unwrap_special_forms(list[int])
    assert result_type == list[int]
    assert result_metadata == Annotations()


def test_union_not_unwrapped():
    result_type, result_metadata = unwrap_special_forms(Union[int, str])
    assert result_type == Union[int, str]
    assert result_metadata == Annotations()
