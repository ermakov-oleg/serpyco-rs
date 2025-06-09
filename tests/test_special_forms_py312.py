from typing import Annotated

from serpyco_rs._meta import Annotations
from serpyco_rs._secial_forms import unwrap_special_forms
from serpyco_rs.metadata import Min


def test_simple_pep_695():
    type Foo = int
    assert unwrap_special_forms(Foo) == (int, Annotations())


def test_generic_pep_695():
    type Foo[T] = list[T]
    assert unwrap_special_forms(Foo[int]) == (list[int], Annotations())
    assert str(unwrap_special_forms(Foo)[0]) == 'list[T]'


def test_pep_695_with_annotated():
    type AnnotatedInt = Annotated[int, Min(5)]
    result_type, result_metadata = unwrap_special_forms(AnnotatedInt)
    assert result_type == int
    assert result_metadata.get(Min) == Min(5)


def test_pep_695_generic_with_annotated():
    type AnnotatedList[T] = Annotated[list[T], Min(1)]
    result_type, result_metadata = unwrap_special_forms(AnnotatedList[str])
    assert str(result_type) == 'list[str]'
    assert result_metadata.get(Min) == Min(1)
