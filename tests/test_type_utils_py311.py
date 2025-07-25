from typing import Generic, Tuple, TypeVar, TypeVarTuple, Unpack

from serpyco_rs._type_utils import get_type_hints


T = TypeVar('T')
U = TypeVar('U')
KT = TypeVar('KT')
VT = TypeVar('VT')


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
