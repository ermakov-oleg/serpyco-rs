from serpyco_rs._type_utils import get_type_hints


def test_get_type_hints_multiple_generic_inheritance():
    class Foo[T, KT]:
        arr: list[T]
        key: KT

    class Bar[T]:
        beer: T

    class Baz[T, KT](Foo[T, KT], Bar[T]):
        pass

    class Baz2[T, KT](Foo[T, KT], Bar[T]):
        pass

    assert get_type_hints(Baz[int, bool]) == {'arr': list[int], 'key': bool, 'beer': int}
    assert get_type_hints(Baz2[int, bool]) == {'arr': list[int], 'key': bool, 'beer': int}


def test_get_type_hints_swapped_typevar_order():
    class Foo[T, U]:
        type: T
        info: U

    class Bar[T](Foo[str, T]):
        test: T

    assert get_type_hints(Bar[int]) == {'type': str, 'info': int, 'test': int}


def test_get_type_hints_deep_generic_inheritance():
    class Foo[T]:
        fld: list[T]

    class Bar[T](Foo[T]):
        pass

    class Baz[T](Bar[T]):
        pass

    assert get_type_hints(Baz[int]) == {
        'fld': list[int],
    }


def test_get_type_hints_deep_generic_inheritance_different_typvar_name():
    class Foo[T]:
        fld: list[T]

    class Bar[B](Foo[B]):
        pass

    class Baz[C](Bar[C]):
        pass

    assert get_type_hints(Baz[int]) == {
        'fld': list[int],
    }


def test_get_type_hints_deep_generic_inheritance_swapped_typevar_order():
    class Foo[T1, T2]:
        fld1: list[T1]
        fld2: list[T2]

    class Bar[T1, T2](Foo[T2, T1]):
        pass

    class Baz[T1, T2](Bar[T1, T2]):
        pass

    assert get_type_hints(Baz[int, str]) == {
        'fld1': list[str],
        'fld2': list[int],
    }


def test_multiple_inheritance_with_generics():
    class Mixin1[T]:
        field1: T

    class Mixin2[U]:
        field2: list[U]

    class Base[T, U](Mixin1[T], Mixin2[U]):
        pass

    class Derived[T, U](Base[T, U]):
        pass

    assert get_type_hints(Derived[int, str]) == {'field1': int, 'field2': list[str]}


def test_swapped_typevar_order():
    class Base[T, U]:
        field1: T
        field2: U

    class Swapped[A, B](Base[B, A]):
        pass

    class Final[X, Y](Swapped[X, Y]):
        pass

    assert get_type_hints(Final[str, int]) == {
        'field1': int,  # B -> Y -> int
        'field2': str,  # A -> X -> str
    }


def test_complex_types():
    class Base[T]:
        optional_field: T | None
        union_field: T | str
        tuple_field: tuple[T, int, T]

    class Middle[T](Base[T]):
        pass

    class Final[T](Middle[T]):
        pass

    assert get_type_hints(Final[bool]) == {
        'optional_field': bool | None,
        'union_field': bool | str,
        'tuple_field': tuple[bool, int, bool],
    }


def test_multiple_generic_bases():
    class Readable[T]:
        read_data: T

    class Writable[U]:
        write_data: U

    class Processable[V]:
        process_data: V

    class MultiBase[T, U, V](Readable[T], Writable[U], Processable[V]):
        pass

    class Final[T, U, V](MultiBase[T, U, V]):
        pass

    assert get_type_hints(Final[int, str, bool]) == {'read_data': int, 'write_data': str, 'process_data': bool}


def test_mixed_variance_inheritance():
    class Base[T, U, V]:
        field1: T
        field2: U
        field3: V

    class DropOne[A, B](Base[A, str, B]):  # U fixed as str
        pass

    class ReorderAndDrop[X](DropOne[int, X]):  # A fixed as int
        pass

    assert get_type_hints(ReorderAndDrop[bool]) == {
        'field1': int,
        'field2': str,
        'field3': bool,
    }
