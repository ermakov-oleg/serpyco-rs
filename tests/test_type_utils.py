from dataclasses import dataclass
from typing import TypeVar, Generic, List, Type, TypeVarTuple, Tuple, Unpack

from serpyco_rs._type_utils import get_type_hints

T = TypeVar('T')
U = TypeVar('U')
KT = TypeVar('KT')
VT = TypeVar('VT')


def test_get_type_hints():
    @dataclass
    class A:
        a: int
        b: str

    assert get_type_hints(A) == {'a': int, 'b': str}


def test_get_type_hints_with_inheritance():
    @dataclass
    class A:
        a: int
        b: str

    @dataclass
    class B(A):
        c: float

    assert get_type_hints(B) == {'a': int, 'b': str, 'c': float}


def test_get_type_hints_with_nested_dataclasses():
    @dataclass
    class A:
        a: int
        b: str

    @dataclass
    class B:
        a: A
        c: float

    assert get_type_hints(B) == {'a': A, 'c': float}


def test_get_type_hints_with_generics():

    @dataclass
    class A(Generic[T, U]):
        a: T
        b: U

    @dataclass
    class B(A[int, U]):
        pass

    assert get_type_hints(B[bool]) == {'a': int, 'b': bool}


def test_get_type_hints_with_generics_and_reuse_type_vars():

    @dataclass
    class A(Generic[T, U]):
        a: T
        b: U

    @dataclass
    class B(A[int, T]):
        pass

    assert get_type_hints(B[str]) == {'a': int, 'b': str}


def test_get_type_hints_with_generics_and_nested_dataclasses():

        @dataclass
        class A(Generic[T, U]):
            a: T
            b: U

        @dataclass
        class B:
            a: A[int, str]
            c: float

        assert get_type_hints(B) == {'a': A[int, str], 'c': float}


def test_get_type_hints_with_generics_and_annotations():

    @dataclass
    class A(Generic[T, U]):
        a: T
        b: U

    @dataclass
    class B:
        f: A[int, str]
        g: A[float, str]

    assert get_type_hints(B) == {'f': A[int, str], 'g': A[float, str]}


def test_get_type_hints_generic_alias():
    class Foo(Generic[T, KT]):
        arr: List[T]
        key: KT

    assert get_type_hints(Foo[str, int]) == {'arr': List[str], 'key': int}


def test_get_type_hints_partial_typevars():
    class Foo(Generic[T, KT]):
        arr: List[T]
        key: KT

    class Bar(Foo[str, KT]):
        pass

    assert get_type_hints(Bar) == {'arr': List[str], 'key': KT}
    assert get_type_hints(Bar[bool]) == {'arr': List[str], 'key': bool}


def test_get_type_hints_swapped_typevar_order():
    class Foo(Generic[T, KT]):
        arr: List[T]
        key: KT

    class Bar(Foo[KT, T]):
        pass

    assert get_type_hints(Bar) == {'arr': List[KT], 'key': T}
    assert get_type_hints(Bar) == {'arr': List[KT], 'key': T}


def test_get_type_hints_multiple_generic_inheritance():
    class Foo(Generic[T, KT]):
        arr: List[T]
        key: KT

    class Bar(Generic[T]):
        beer: T

    class Baz(Foo[T, KT], Bar[T], Generic[T, KT]):
        pass

    class Baz2(Foo[T, KT], Bar[T]):
        pass

    assert get_type_hints(Baz[int, bool]) == {'arr': List[int], 'key': bool, 'beer': int}
    assert get_type_hints(Baz2[int, bool]) == {'arr': List[int], 'key': bool, 'beer': int}


def test_get_type_hints_unorthodox_generic_placement():
    class A(Generic[T, VT]):
        a_type: T
        value: VT

    class B(Generic[KT, T]):
        b_type: T
        key: KT

    class C(A[T, VT], Generic[VT, T, KT], B[KT, T]):
        pass

    assert get_type_hints(C) == {'a_type': T, 'value': VT, 'b_type': T, 'key': KT}


def test_get_type_hints_extended_generic_rules_subclassing():
    class T1(Type[T]):
        value: T

    class T2(list[T]):
        value: T

    assert get_type_hints(T1) == {'value': T}
    assert get_type_hints(T2) == {'value': T}
    assert get_type_hints(T1[bool]) == {'value': bool}


def test_get_type_hints_generic_class_no_params_required():
    class Foo(Generic[T, KT]):
        arr: List[T]
        key: KT

    class Bar(Foo[str, int]):
        pass

    assert get_type_hints(Bar) == {'arr': List[str], 'key': int}


def test_get_type_hints_unbound_typevar():
    # https://github.com/python/cpython/pull/111515#issuecomment-2018336687

    class Foo(Generic[T]):
        x: list[T]
        y: KT

    assert get_type_hints(Foo) == {'x': list[T], 'y': KT}  # ok
    assert get_type_hints(Foo[int]) == {'x': list[int], 'y': KT}    # error


def test_get_type_hints_variadic_class():
    T1 = TypeVar('T1')
    T2 = TypeVar('T2')
    Ts = TypeVarTuple('Ts')

    class A(Generic[T1, T2, *Ts]):
        t1: T1
        t2: T2
        ts: Tuple[*Ts]
    assert get_type_hints(A[int, str]) == {'t1': int, 't2': str, 'ts': Tuple[()]}
    assert get_type_hints(A[int, str, float]) == {'t1': int, 't2': str, 'ts': Tuple[float]}
    assert get_type_hints(A[int, str, float, bool]) == {'t1': int, 't2': str, 'ts': Tuple[float, bool]}

    class B(Generic[T1, T2, Unpack[Ts]]):
        t1: T1
        t2: T2
        ts: Tuple[Unpack[Ts]]

    assert get_type_hints(B[int, str]) == {'t1': int, 't2': str, 'ts': Tuple[()]}
    assert get_type_hints(B[int, str, float]) == {'t1': int, 't2': str, 'ts': Tuple[float]}
    assert get_type_hints(B[int, str, float, bool]) == {'t1': int, 't2': str, 'ts': Tuple[float, bool]}

    class C(Generic[T1, *Ts, T2]):
        t1: T1
        ts: Tuple[*Ts]
        t2: T2

    assert get_type_hints(C[int, str]) == {'t1': int, 'ts': Tuple[()], 't2': str}
    assert get_type_hints(C[int, str, float]) == {'t1': int, 'ts': Tuple[str], 't2': float}
    assert get_type_hints(C[int, str, float, bool]) == {'t1': int, 'ts': Tuple[str, float], 't2': bool}

    class E(Generic[*Ts, T1, T2]):
        ts: Tuple[*Ts]
        t1: T1
        t2: T2

    assert get_type_hints(E[int, str]) == {'ts': Tuple[()], 't1': int, 't2': str}
    assert get_type_hints(E[int, str, float]) == {'ts': Tuple[int], 't1': str, 't2': float}
    assert get_type_hints(E[int, str, float, bool]) == {'ts': Tuple[int, str], 't1': float, 't2': bool}


def test_get_type_hints_variadic_type_parameter():
    T1 = TypeVar('T1')
    T2 = TypeVar('T2')
    Ts = TypeVarTuple('Ts')

    class A(Generic[T1, T2, *Ts]):
        t1: T1
        t2: T2
        ts: Tuple[*Ts]

    class B(A[int, str, *Ts]):
        pass

    assert get_type_hints(B) == {'t1': int, 't2': str, 'ts': Tuple[*Ts]}
    assert get_type_hints(B[()]) == {'t1': int, 't2': str, 'ts': Tuple[()]}
    assert get_type_hints(B[float]) == {'t1': int, 't2': str, 'ts': Tuple[float]}


def test_get_type_hints_variadic_with_typevars():
    T1 = TypeVar('T1')
    T2 = TypeVar('T2')
    Ts = TypeVarTuple('Ts')

    class A(Generic[T1, T2, *Ts]):
        t1: T1
        t2: T2
        ts: Tuple[*Ts]

    class D(A[int, str, T1, T2]):
        pass

    assert get_type_hints(D[int, str]) == {'t1': int, 't2': str, 'ts': Tuple[int, str]}
