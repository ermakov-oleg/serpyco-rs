use std::fmt::Display;
use pyo3::{PyErr, PyResult};
use crate::validator::{InstancePath, raise_error, Value};
use crate::validator::Context;


pub fn check_lower_bound<T>(val: T, min: Option<T>, instance_path: &InstancePath) -> PyResult<()>
where T: PartialOrd + Display
{
    if let Some(min) = min {
        if val <= min {
            raise_error(
                format!("{} is less than the minimum of {}", val, min),
                instance_path,
            )?;
        }
    }
    Ok(())
}


pub fn check_upper_bound<T>(val: T, max: Option<T>, instance_path: &InstancePath) -> PyResult<()>
where T: PartialOrd + Display
{
    if let Some(max) = max {
        if val > max {
            raise_error(
                format!("{} is greater than the maximum of {}", val, max),
                instance_path,
            )?;
        }
    }
    Ok(())
}

pub fn check_min_length(val: &str, min: Option<usize>, instance_path: &InstancePath) -> PyResult<()> {
    if let Some(min) = min {
        if val.len() <= min {
            raise_error(
                format!(r#""{}" is shorter than {} characters"#, val, min),
                instance_path,
            )?;
        }
    }
    Ok(())
}

pub fn check_max_length(val: &str, max: Option<usize>, instance_path: &InstancePath) -> PyResult<()> {
    if let Some(max) = max {
        if val.len() > max {
            raise_error(
                format!(r#""{}" is longer than {} characters"#, val, max),
                instance_path,
            )?;
        }
    }
    Ok(())
}

pub fn missing_required_property(property: &str, instance_path: &InstancePath) -> PyErr {
    raise_error(format!(r#""{}" is a required property"#, property), instance_path).unwrap_err()
}


pub fn _invalid_type(type_: &str, value: Value, instance_path: &InstancePath) -> PyResult<()> {
    let error = match value.as_str() {
        Some(val) => format!(r#""{}" is not of type "{}""#, val, type_),
        None => format!(r#"{} is not of type "{}""#, value.to_string()?, type_),
    };
    raise_error(error, instance_path)?;
    Ok(())
}

macro_rules! invalid_type {
    ($type_: expr, $value: expr, $ctx: expr) => {
        {
            crate::validator::validators::_invalid_type($type_, $value, $ctx)?;
            unreachable!();  // todo: Discard the use of unreachable
        }
    }
}
pub(crate) use invalid_type;


pub enum ValidationErrorKind {
    /// The input array contain more items than expected.
    AdditionalItems { limit: usize },
    /// Unexpected properties.
    AdditionalProperties { unexpected: Vec<String> },
    /// The input value is not valid under any of the schemas listed in the 'anyOf' keyword.
    AnyOf,
    /// The input value doesn't match expected constant.
    Constant { expected_value: Value },
    /// The input array doesn't contain items conforming to the specified schema.
    Contains,
    /// The input value does not respect the defined contentEncoding
    ContentEncoding { content_encoding: String },
    /// The input value does not respect the defined contentMediaType
    ContentMediaType { content_media_type: String },
    /// The input value doesn't match any of specified options.
    Enum { options: Value },
    /// Value is too large.
    ExclusiveMaximum { limit: Value },
    /// Value is too small.
    ExclusiveMinimum { limit: Value },
    /// When the input doesn't match to the specified format.
    Format { format: &'static str },
    /// May happen in `contentEncoding` validation if `base64` encoded data is invalid.
    // FromUtf8 { error: FromUtf8Error },
    /// Invalid UTF-8 string during percent encoding when resolving happens
    // Utf8 { error: Utf8Error },
    /// May happen during ref resolution when remote document is not a valid JSON.
    JSONParse { error: serde_json::Error },
    /// `ref` value is not valid.
    InvalidReference { reference: String },
    /// Invalid URL, e.g. invalid port number or IP address
    // InvalidURL { error: url::ParseError },
    /// Too many items in an array.
    MaxItems { limit: u64 },
    /// Value is too large.
    Maximum { limit: Value },  // done
    /// String is too long.
    MaxLength { limit: u64 },
    /// Too many properties in an object.
    MaxProperties { limit: u64 },
    /// Too few items in an array.
    MinItems { limit: u64 },
    /// Value is too small.
    Minimum { limit: Value }, // done
    /// String is too short.
    MinLength { limit: u64 },
    /// Not enough properties in an object.
    MinProperties { limit: u64 },
    /// When some number is not a multiple of another number.
    MultipleOf { multiple_of: f64 },
    /// Negated schema failed validation.
    Not { schema: Value },
    /// The given schema is valid under more than one of the schemas listed in the 'oneOf' keyword.
    OneOfMultipleValid,
    /// The given schema is not valid under any of the schemas listed in the 'oneOf' keyword.
    OneOfNotValid,
    /// When the input doesn't match to a pattern.
    Pattern { pattern: String },
    /// Object property names are invalid.
    // PropertyNames {
    //     error: Box<ValidationError<'static>>,
    // },
    /// When a required property is missing.
    Required { property: Value },
    /// Resolved schema failed to compile.
    Schema,
    /// When the input value doesn't match one or multiple required types.
    // Type { kind: TypeKind },
    /// Unexpected properties.
    UnevaluatedProperties { unexpected: Vec<String> },
    /// When the input array has non-unique elements.
    UniqueItems,
}
