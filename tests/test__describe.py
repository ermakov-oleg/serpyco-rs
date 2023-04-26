import sys
from collections.abc import Mapping
from dataclasses import dataclass, field
from datetime import date, datetime, time
from decimal import Decimal
from enum import Enum
from typing import Annotated, Any, Generic, Literal, Optional, Sequence, TypeVar, Union
from unittest import mock
from unittest.mock import ANY
from uuid import UUID

import attr
import pytest
from serpyco_rs._describe import (
    NOT_SET,
    AnyType,
    ArrayType,
    BooleanType,
    DateTimeType,
    DateType,
    DecimalType,
    DictionaryType,
    EntityField,
    EntityType,
    EnumType,
    FloatType,
    IntegerType,
    LiteralType,
    OptionalType,
    StringType,
    TimeType,
    TupleType,
    UnionType,
    UUIDType,
    describe_type,
)
from serpyco_rs.metadata import CamelCase, Discriminator, Max, MaxLength, Min, MinLength, NoFormat, Places

T = TypeVar("T")
U = TypeVar("U")


def test_describe__dataclass__supported():
    class SomeEnum(Enum):
        a = "a"

    @dataclass
    class SomeOtherEntity:
        a: int

    @dataclass
    class SomeEntity:
        """Doc"""

        a: int
        b: float
        c: Decimal
        d: bool
        e: str
        f: UUID
        g: time
        h: date
        i: datetime
        j: SomeEnum
        k: SomeOtherEntity
        l: list[int]
        m: Sequence[int]
        n: dict[str, int]
        o: Mapping[str, int]
        p: Any

    assert describe_type(SomeEntity) == EntityType(
        cls=SomeEntity,
        name=ANY,
        fields=[
            EntityField(
                name="a",
                type=IntegerType(min=None, max=None, custom_encoder=None),
                doc=None,
                default=NOT_SET,
                default_factory=NOT_SET,
                is_property=False,
                dict_key="a",
            ),
            EntityField(
                name="b",
                type=FloatType(min=None, max=None, custom_encoder=None),
                doc=None,
                default=NOT_SET,
                default_factory=NOT_SET,
                is_property=False,
                dict_key="b",
            ),
            EntityField(
                name="c",
                type=DecimalType(places=None, min=None, max=None, custom_encoder=None),
                doc=None,
                default=NOT_SET,
                default_factory=NOT_SET,
                is_property=False,
                dict_key="c",
            ),
            EntityField(
                name="d",
                type=BooleanType(custom_encoder=None),
                doc=None,
                default=NOT_SET,
                default_factory=NOT_SET,
                is_property=False,
                dict_key="d",
            ),
            EntityField(
                name="e",
                type=StringType(min_length=None, max_length=None, custom_encoder=None),
                doc=None,
                default=NOT_SET,
                default_factory=NOT_SET,
                is_property=False,
                dict_key="e",
            ),
            EntityField(
                name="f",
                type=UUIDType(custom_encoder=None),
                doc=None,
                default=NOT_SET,
                default_factory=NOT_SET,
                is_property=False,
                dict_key="f",
            ),
            EntityField(
                name="g",
                type=TimeType(custom_encoder=None),
                doc=None,
                default=NOT_SET,
                default_factory=NOT_SET,
                is_property=False,
                dict_key="g",
            ),
            EntityField(
                name="h",
                type=DateType(custom_encoder=None),
                doc=None,
                default=NOT_SET,
                default_factory=NOT_SET,
                is_property=False,
                dict_key="h",
            ),
            EntityField(
                name="i",
                type=DateTimeType(custom_encoder=None),
                doc=None,
                default=NOT_SET,
                default_factory=NOT_SET,
                is_property=False,
                dict_key="i",
            ),
            EntityField(
                name="j",
                type=EnumType(cls=SomeEnum, custom_encoder=None),
                doc=None,
                default=NOT_SET,
                default_factory=NOT_SET,
                is_property=False,
                dict_key="j",
            ),
            EntityField(
                name="k",
                type=EntityType(
                    cls=SomeOtherEntity,
                    name=ANY,
                    fields=[
                        EntityField(
                            name="a",
                            type=IntegerType(min=None, max=None, custom_encoder=None),
                            doc=None,
                            default=NOT_SET,
                            default_factory=NOT_SET,
                            is_property=False,
                            dict_key="a",
                        )
                    ],
                    generics={},
                    doc="SomeOtherEntity(a: int)",
                    custom_encoder=None,
                ),
                doc=None,
                default=NOT_SET,
                default_factory=NOT_SET,
                is_property=False,
                dict_key="k",
            ),
            EntityField(
                name="l",
                type=ArrayType(
                    item_type=IntegerType(min=None, max=None, custom_encoder=None),
                    is_sequence=False,
                    custom_encoder=None,
                ),
                doc=None,
                default=NOT_SET,
                default_factory=NOT_SET,
                is_property=False,
                dict_key="l",
            ),
            EntityField(
                name="m",
                type=ArrayType(
                    item_type=IntegerType(min=None, max=None, custom_encoder=None),
                    is_sequence=True,
                    custom_encoder=None,
                ),
                doc=None,
                default=NOT_SET,
                default_factory=NOT_SET,
                is_property=False,
                dict_key="m",
            ),
            EntityField(
                name="n",
                type=DictionaryType(
                    key_type=StringType(min_length=None, max_length=None, custom_encoder=None),
                    value_type=IntegerType(min=None, max=None, custom_encoder=None),
                    is_mapping=False,
                    custom_encoder=None,
                ),
                doc=None,
                default=NOT_SET,
                default_factory=NOT_SET,
                is_property=False,
                dict_key="n",
            ),
            EntityField(
                name="o",
                type=DictionaryType(
                    key_type=StringType(min_length=None, max_length=None, custom_encoder=None),
                    value_type=IntegerType(min=None, max=None, custom_encoder=None),
                    is_mapping=True,
                    custom_encoder=None,
                ),
                doc=None,
                default=NOT_SET,
                default_factory=NOT_SET,
                is_property=False,
                dict_key="o",
            ),
            EntityField(
                name="p",
                type=AnyType(custom_encoder=None),
                doc=None,
                default=NOT_SET,
                default_factory=NOT_SET,
                is_property=False,
                dict_key="p",
            ),
        ],
        generics={},
        doc="Doc",
        custom_encoder=None,
    )


def test_describe_dataclass__dict_type__works_without_type_parameters():
    @dataclass
    class SomeEntity:
        x: dict

    assert describe_type(SomeEntity).fields[0].type == DictionaryType(
        key_type=AnyType(custom_encoder=None),
        value_type=AnyType(custom_encoder=None),
        is_mapping=False,
        custom_encoder=None,
    )


def test_describe_dataclass__list_type__works_without_type_parameters():
    @dataclass
    class SomeEntity:
        x: list

    assert describe_type(SomeEntity).fields[0].type == ArrayType(
        item_type=AnyType(custom_encoder=None), is_sequence=False, custom_encoder=None
    )


def test_describe_dataclass__field_has_docstring__doc_filled():
    @dataclass
    class SomeEntity:
        x: int
        """Foo"""

    assert describe_type(SomeEntity).fields[0].doc == "Foo"


def test_describe_dataclass__has_default__default_filled():
    @dataclass
    class SomeEntity:
        x: int = 1

    assert describe_type(SomeEntity).fields[0].default == 1


def test_describe_dataclass__has_default_factory__default_factory_filled():
    def factory():
        return 1

    @dataclass
    class SomeEntity:
        x: int = field(default_factory=factory)

    assert describe_type(SomeEntity).fields[0].default is NOT_SET
    assert describe_type(SomeEntity).fields[0].default_factory is factory


def test_describe_dataclass__generic_but_without_type_vars__filled_by_any():
    @dataclass
    class SomeEntity(Generic[T]):
        x: list[T]

    result: EntityType = describe_type(SomeEntity)
    assert result.fields[0].type == ArrayType(
        item_type=AnyType(custom_encoder=None), is_sequence=False, custom_encoder=None
    )
    assert result.generics == {T: Any}


def test_describe_dataclass__generic_with_type_params__expected_right_type():
    @dataclass
    class SomeOtherEntity(Generic[T]):
        x: T

    @dataclass
    class SomeEntity(Generic[T]):
        x: list[T]
        y: SomeOtherEntity[T]

    result: EntityType = describe_type(SomeEntity[int])
    assert result.fields[0].type == ArrayType(
        item_type=IntegerType(custom_encoder=None), is_sequence=False, custom_encoder=None
    )
    assert result.fields[1].type == EntityType(
        cls=SomeOtherEntity,
        name=ANY,
        generics={T: int},
        fields=[EntityField(name="x", type=IntegerType(custom_encoder=None), dict_key="x")],
        doc="SomeOtherEntity(x: ~T)",
        custom_encoder=None,
    )


def test_describe_dataclass__used_unknown_type_var__fail():
    @dataclass
    class SomeEntity(Generic[T]):
        x: list[T]
        y: U

    with pytest.raises(RuntimeError) as exc_info:
        describe_type(SomeEntity[int])

    assert exc_info.match("Unfilled TypeVar: ~U")


def test_describe__dataclass_with_forward_ref_annotation__parsed():
    @dataclass
    class SomeEntity:
        x: "int"

    assert describe_type(SomeEntity).fields[0].type == IntegerType(custom_encoder=None)


def test_describe__dataclass_with_invalid_forward_ref_annotation__parsed():
    @dataclass
    class SomeEntity:
        x: "intt"

    with pytest.raises(NameError) as exc_info:
        describe_type(SomeEntity)

    assert exc_info.match(r"name 'intt' is not defined")


def test_describe__dataclass_and_annotated_with_min_max__parsed():
    @dataclass
    class SomeEntity:
        x: Annotated[int, Min(10), Max(20)]

    result = describe_type(SomeEntity)

    assert result == EntityType(
        cls=SomeEntity,
        name=ANY,
        fields=[
            EntityField(
                name="x",
                dict_key="x",
                type=IntegerType(min=10, max=20, custom_encoder=None),
            )
        ],
        doc="SomeEntity(x: typing.Annotated[int, Min(value=10), Max(value=20)])",
        custom_encoder=None,
    )


def test_describe__dataclass_and_annotated_with_min_max_length__parsed():
    @dataclass
    class SomeEntity:
        x: Annotated[str, MinLength(10), MaxLength(20)]

    result = describe_type(SomeEntity)

    assert result == EntityType(
        cls=SomeEntity,
        name=ANY,
        fields=[
            EntityField(
                name="x",
                dict_key="x",
                type=StringType(min_length=10, max_length=20, custom_encoder=None),
            )
        ],
        doc="SomeEntity(x: typing.Annotated[str, MinLength(value=10), MaxLength(value=20)])",
        custom_encoder=None,
    )


def test_describe__attrs_and_field_has_docstring__doc_filled():
    @attr.s(auto_attribs=True)
    class SomeEntity:
        x: int
        """Foo"""

    assert describe_type(SomeEntity).fields[0].doc == "Foo"


def test_describe__attrs_and_has_default__default_filled():
    @attr.s(auto_attribs=True)
    class SomeEntity:
        x: int = 1

    assert describe_type(SomeEntity).fields[0].default == 1


def test_describe__attrs_and_has_default_factory__default_factory_filled():
    def factory():
        return 1

    @attr.s(auto_attribs=True)
    class SomeEntity:
        x: int = attr.ib(default=attr.Factory(factory))
        y: int = attr.ib(factory=factory)

    assert describe_type(SomeEntity).fields[0].default is NOT_SET
    assert describe_type(SomeEntity).fields[0].default_factory is factory
    assert describe_type(SomeEntity).fields[1].default is NOT_SET
    assert describe_type(SomeEntity).fields[1].default_factory is factory


def test_describe__attrs_with_forward_ref_annotation__parsed():
    @attr.s(auto_attribs=True)
    class SomeEntity:
        x: "int"

    assert describe_type(SomeEntity).fields[0].type == IntegerType(custom_encoder=None)


def test_describe__attrs_with_invalid_forward_ref_annotation__parsed():
    @attr.s(auto_attribs=True)
    class SomeEntity:
        x: "intt"

    with pytest.raises(NameError) as exc_info:
        describe_type(SomeEntity)

    assert exc_info.match(r"name 'intt' is not defined")


def test_describe__attrs_and_annotated_with_min_max__parsed():
    @attr.define
    class SomeEntity:
        x: Annotated[int, Min(10), Max(20)]

    result = describe_type(SomeEntity)

    assert result == EntityType(
        cls=SomeEntity,
        fields=[
            EntityField(
                name="x",
                dict_key="x",
                type=IntegerType(min=10, max=20, custom_encoder=None),
            )
        ],
        name=ANY,
        custom_encoder=None,
    )


def test_describe__attrs_and_annotated_with_min_max_length__parsed():
    @attr.define
    class SomeEntity:
        x: Annotated[str, MinLength(10), MaxLength(20)]

    result = describe_type(SomeEntity)

    assert result == EntityType(
        cls=SomeEntity,
        name=ANY,
        custom_encoder=None,
        fields=[
            EntityField(
                name="x",
                dict_key="x",
                type=StringType(min_length=10, max_length=20, custom_encoder=None),
            )
        ],
    )


def test_describe__type_with_typevar__fail():
    with pytest.raises(RuntimeError) as exc_info:
        describe_type(list[T])

    assert exc_info.match("Unfilled TypeVar: ~T")


def test_describe__unknown_type__fail():
    with pytest.raises(RuntimeError) as exc_info:
        describe_type(set[int])

    assert exc_info.match("Unknown type <class 'set'>")


def test_describe__optional__wrapped():
    assert describe_type(Optional[int]) == OptionalType(inner=IntegerType(custom_encoder=None), custom_encoder=None)


def test_describe__other_unions__error():
    with pytest.raises(RuntimeError) as exc_info:
        describe_type(Union[int, str])

    assert exc_info.match("For support Unions need specify serpyco_rs.metadata.Discriminator")


@pytest.mark.skipif(sys.version_info < (3, 10), reason="New style unions available after 3.10")
def test_describe__new_style_union_type__wrapped():
    assert describe_type(int | None) == OptionalType(inner=IntegerType(custom_encoder=None), custom_encoder=None)


def test_describe__tuple__parsed():
    assert describe_type(tuple[int, str]) == TupleType(
        item_types=[IntegerType(custom_encoder=None), StringType(custom_encoder=None)],
        custom_encoder=None,
    )


@pytest.mark.parametrize("t", [tuple, tuple[int, ...]])
def test_describe__invalid_tuple__error(t):
    with pytest.raises(RuntimeError) as exc_info:
        describe_type(t)

    assert exc_info.match("Variable length tuples are not supported")


def test_describe__decimal_with_places__parsed():
    assert describe_type(Annotated[Decimal, Places(3)]) == DecimalType(places=3, custom_encoder=None)


def test_describe__dataclass_field_format__parsed():
    @dataclass
    class InnerEntity:
        foo_field: str
        bar_field: int

    @dataclass
    class Entity:
        inner_entity: Annotated[list[InnerEntity], NoFormat]
        some_filed: str

    assert describe_type(Annotated[Entity, CamelCase]) == EntityType(
        cls=Entity,
        name=ANY,
        custom_encoder=None,
        fields=[
            EntityField(
                name="inner_entity",
                dict_key="innerEntity",
                type=ArrayType(
                    is_sequence=False,
                    custom_encoder=None,
                    item_type=EntityType(
                        name=ANY,
                        cls=InnerEntity,
                        fields=[
                            EntityField(
                                name="foo_field",
                                dict_key="foo_field",
                                type=StringType(custom_encoder=None),
                            ),
                            EntityField(
                                name="bar_field",
                                dict_key="bar_field",
                                type=IntegerType(custom_encoder=None),
                            ),
                        ],
                        doc=mock.ANY,
                        custom_encoder=None,
                    ),
                ),
            ),
            EntityField(
                name="some_filed",
                dict_key="someFiled",
                type=StringType(custom_encoder=None),
            ),
        ],
        doc=mock.ANY,
    )


def test_describe__literal():
    assert describe_type(Literal["foo", "bar"]) == LiteralType(args=("foo", "bar"), custom_encoder=None)


def test_describe__tagged_union():
    @dataclass
    class Foo:
        val: int
        filed_type: Literal["foo"]

    @dataclass
    class Bar:
        val: str
        filed_type: Literal["bar"]

    @dataclass
    class Base:
        field: Annotated[Union[Foo, Bar], Discriminator("filed_type"), CamelCase]

    assert describe_type(Base) == EntityType(
        cls=Base,
        name=mock.ANY,
        fields=[
            EntityField(
                name="field",
                dict_key="field",
                type=UnionType(
                    item_types={
                        "foo": EntityType(
                            cls=Foo,
                            name=mock.ANY,
                            fields=[
                                EntityField(
                                    name="val",
                                    dict_key="val",
                                    type=IntegerType(min=None, max=None, custom_encoder=None),
                                ),
                                EntityField(
                                    name="filed_type",
                                    dict_key="filedType",
                                    type=LiteralType(args=("foo",), custom_encoder=None),
                                    is_discriminator_field=True,
                                ),
                            ],
                            doc=mock.ANY,
                            custom_encoder=None,
                        ),
                        "bar": EntityType(
                            cls=Bar,
                            name=mock.ANY,
                            fields=[
                                EntityField(
                                    name="val",
                                    dict_key="val",
                                    type=StringType(min_length=None, max_length=None, custom_encoder=None),
                                ),
                                EntityField(
                                    name="filed_type",
                                    dict_key="filedType",
                                    type=LiteralType(args=("bar",), custom_encoder=None),
                                    is_discriminator_field=True,
                                ),
                            ],
                            doc=mock.ANY,
                            custom_encoder=None,
                        ),
                    },
                    load_discriminator="filedType",
                    dump_discriminator="filed_type",
                    custom_encoder=None,
                ),
            )
        ],
        doc=mock.ANY,
        custom_encoder=None,
    )
