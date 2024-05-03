mod errors;
mod python;
mod serializer;
mod validator;

use pyo3::prelude::*;
use validator::types;

#[pymodule]
fn _serpyco_rs(py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<serializer::Serializer>()?;

    // Types
    m.add_class::<types::CustomEncoder>()?;
    m.add_class::<types::BaseType>()?;
    m.add_class::<types::IntegerType>()?;
    m.add_class::<types::StringType>()?;
    m.add_class::<types::FloatType>()?;
    m.add_class::<types::DecimalType>()?;
    m.add_class::<types::BooleanType>()?;
    m.add_class::<types::UUIDType>()?;
    m.add_class::<types::TimeType>()?;
    m.add_class::<types::DateTimeType>()?;
    m.add_class::<types::DateType>()?;
    m.add_class::<types::EntityType>()?;
    m.add_class::<types::TypedDictType>()?;
    m.add_class::<types::EntityField>()?;
    m.add_class::<types::DefaultValue>()?;
    m.add_class::<types::ArrayType>()?;
    m.add_class::<types::EnumType>()?;
    m.add_class::<types::OptionalType>()?;
    m.add_class::<types::DictionaryType>()?;
    m.add_class::<types::TupleType>()?;
    m.add_class::<types::BytesType>()?;
    m.add_class::<types::AnyType>()?;
    m.add_class::<types::UnionType>()?;
    m.add_class::<types::DiscriminatedUnionType>()?;
    m.add_class::<types::LiteralType>()?;
    m.add_class::<types::RecursionHolder>()?;
    m.add_class::<types::CustomType>()?;

    // Errors
    m.add(
        "ValidationError",
        py.get_type_bound::<errors::ValidationError>(),
    )?;
    m.add(
        "SchemaValidationError",
        py.get_type_bound::<errors::SchemaValidationError>(),
    )?;
    m.add("ErrorItem", py.get_type_bound::<errors::ErrorItem>())?;
    Ok(())
}
