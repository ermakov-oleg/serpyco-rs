use pyo3::prelude::PyAnyMethods;
use pyo3::Bound;
use pyo3::{PyAny, PyResult};

use crate::validator::types::{
    AnyType, ArrayType, BaseType, BooleanType, BytesType, CustomType, DateTimeType, DateType,
    DecimalType, DictionaryType, DiscriminatedUnionType, EntityType, EnumType, FloatType,
    IntegerType, LiteralType, OptionalType, RecursionHolder, StringType, TimeType, TupleType,
    TypedDictType, UUIDType, UnionType,
};

#[derive(Clone, Debug)]
pub enum Type<'a, Base = Bound<'a, BaseType>> {
    Integer(Bound<'a, IntegerType>, Base),
    Float(Bound<'a, FloatType>, Base),
    Decimal(Bound<'a, DecimalType>, Base),
    String(Bound<'a, StringType>, Base),
    #[allow(dead_code)]
    Boolean(Bound<'a, BooleanType>, Base),
    #[allow(dead_code)]
    Uuid(Bound<'a, UUIDType>, Base),
    #[allow(dead_code)]
    Bytes(Bound<'a, BytesType>, Base),
    #[allow(dead_code)]
    Time(Bound<'a, TimeType>, Base),
    #[allow(dead_code)]
    DateTime(Bound<'a, DateTimeType>, Base),
    #[allow(dead_code)]
    Date(Bound<'a, DateType>, Base),
    Entity(Bound<'a, EntityType>, Base, usize),
    TypedDict(Bound<'a, TypedDictType>, Base, usize),
    Array(Bound<'a, ArrayType>, Base, usize),
    Enum(Bound<'a, EnumType>, Base),
    Optional(Bound<'a, OptionalType>, Base, usize),
    Dictionary(Bound<'a, DictionaryType>, Base, usize),
    Tuple(Bound<'a, TupleType>, Base, usize),
    DiscriminatedUnion(Bound<'a, DiscriminatedUnionType>, Base, usize),
    Union(Bound<'a, UnionType>, Base, usize),
    Literal(Bound<'a, LiteralType>, Base),
    #[allow(dead_code)]
    Any(Bound<'a, AnyType>, Base),
    RecursionHolder(Bound<'a, RecursionHolder>, Base),
    #[allow(dead_code)]
    Custom(Bound<'a, CustomType>, Base),
}

pub fn get_object_type<'a>(type_info: &Bound<'a, PyAny>) -> PyResult<Type<'a>> {
    let base_type = type_info.extract::<Bound<'_, BaseType>>()?;
    let python_object_id = type_info.as_ptr() as *const _ as usize;
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
    check_type!(type_info, base_type, Literal, LiteralType);
    check_type!(type_info, base_type, Bytes, BytesType);
    check_type!(type_info, base_type, RecursionHolder, RecursionHolder);
    check_type!(type_info, base_type, Custom, CustomType);
    check_type!(type_info, base_type, Any, AnyType);
    check_type!(
        type_info,
        base_type,
        Optional,
        OptionalType,
        python_object_id
    );
    check_type!(type_info, base_type, Array, ArrayType, python_object_id);
    check_type!(
        type_info,
        base_type,
        Dictionary,
        DictionaryType,
        python_object_id
    );
    check_type!(type_info, base_type, Tuple, TupleType, python_object_id);
    check_type!(type_info, base_type, Union, UnionType, python_object_id);
    check_type!(
        type_info,
        base_type,
        DiscriminatedUnion,
        DiscriminatedUnionType,
        python_object_id
    );

    check_type!(type_info, base_type, Entity, EntityType, python_object_id);
    check_type!(
        type_info,
        base_type,
        TypedDict,
        TypedDictType,
        python_object_id
    );

    unreachable!("Unknown type: {:?}", type_info)
}

macro_rules! check_type {
    ($type_info:ident, $base_type:ident, $enum:ident, $type:ident) => {
        if let Ok(t) = $type_info.extract::<Bound<'_, $type>>() {
            return Ok(Type::$enum(t, $base_type));
        }
    };
    ($type_info:ident, $base_type:ident, $enum:ident, $type:ident, $python_object_id:ident) => {
        if let Ok(t) = $type_info.extract::<Bound<'_, $type>>() {
            return Ok(Type::$enum(t, $base_type, $python_object_id));
        }
    };
}

pub(crate) use check_type;
