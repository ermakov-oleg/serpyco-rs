use pyo3::prelude::PyAnyMethods;
use pyo3::Bound;
use pyo3::{PyAny, PyResult};

use crate::validator::types::{
    AnyType, ArrayType, BaseType, BooleanType, BytesType, DateTimeType, DateType, DecimalType,
    DictionaryType, DiscriminatedUnionType, EntityType, EnumType, FloatType, IntegerType,
    LiteralType, OptionalType, RecursionHolder, StringType, TimeType, TupleType, TypedDictType,
    UUIDType, UnionType,
};

#[derive(Clone, Debug)]
pub enum Type<'a, Base = Bound<'a, BaseType>> {
    Integer(Bound<'a, IntegerType>, Base),
    Float(Bound<'a, FloatType>, Base),
    Decimal(Bound<'a, DecimalType>, Base),
    String(Bound<'a, StringType>, Base),
    Boolean(Bound<'a, BooleanType>, Base),
    Uuid(Bound<'a, UUIDType>, Base),
    Bytes(Bound<'a, BytesType>, Base),
    Time(Bound<'a, TimeType>, Base),
    DateTime(Bound<'a, DateTimeType>, Base),
    Date(Bound<'a, DateType>, Base),
    Entity(Bound<'a, EntityType>, Base, usize),
    TypedDict(Bound<'a, TypedDictType>, Base, usize),
    Array(Bound<'a, ArrayType>, Base),
    Enum(Bound<'a, EnumType>, Base),
    Optional(Bound<'a, OptionalType>, Base),
    Dictionary(Bound<'a, DictionaryType>, Base),
    Tuple(Bound<'a, TupleType>, Base),
    DiscriminatedUnion(Bound<'a, DiscriminatedUnionType>, Base),
    Union(Bound<'a, UnionType>, Base),
    Literal(Bound<'a, LiteralType>, Base),
    Any(Bound<'a, AnyType>, Base),
    RecursionHolder(Bound<'a, RecursionHolder>, Base),
}

pub fn get_object_type<'a>(type_info: &Bound<'a, PyAny>) -> PyResult<Type<'a>> {
    let base_type = type_info.extract::<Bound<'_, BaseType>>()?;
    check_type!(type_info, base_type, Integer, IntegerType);
    check_type!(type_info, base_type, String, StringType);
    check_type!(type_info, base_type, Float, FloatType);
    check_type!(type_info, base_type, Decimal, DecimalType);
    check_type!(type_info, base_type, Boolean, BooleanType);
    check_type!(type_info, base_type, Uuid, UUIDType);
    check_type!(type_info, base_type, Time, TimeType);
    check_type!(type_info, base_type, DateTime, DateTimeType);
    check_type!(type_info, base_type, Date, DateType);
    check_type!(type_info, base_type, Enum, EnumType);
    check_type!(type_info, base_type, Optional, OptionalType);
    check_type!(type_info, base_type, Array, ArrayType);
    check_type!(type_info, base_type, Dictionary, DictionaryType);
    check_type!(type_info, base_type, Tuple, TupleType);
    check_type!(type_info, base_type, Any, AnyType);
    check_type!(type_info, base_type, Union, UnionType);
    check_type!(
        type_info,
        base_type,
        DiscriminatedUnion,
        DiscriminatedUnionType
    );
    check_type!(type_info, base_type, Literal, LiteralType);
    check_type!(type_info, base_type, Bytes, BytesType);
    check_type!(type_info, base_type, RecursionHolder, RecursionHolder);

    if let Ok(t) = type_info.extract::<Bound<'_, EntityType>>() {
        let python_object_id = type_info.as_ptr() as *const _ as usize;
        Ok(Type::Entity(t, base_type, python_object_id))
    } else if let Ok(t) = type_info.extract::<Bound<'_, TypedDictType>>() {
        let python_object_id = type_info.as_ptr() as *const _ as usize;
        Ok(Type::TypedDict(t, base_type, python_object_id))
    } else {
        unreachable!("Unknown type: {:?}", type_info)
    }
}

macro_rules! check_type {
    ($type_info:ident, $base_type:ident, $enum:ident, $type:ident) => {
        if let Ok(t) = $type_info.extract::<Bound<'_, $type>>() {
            return Ok(Type::$enum(t, $base_type));
        }
    };
}

pub(crate) use check_type;
