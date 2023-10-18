use crate::validator::types::EnumItems;
use crate::validator::{raise_error, InstancePath, Value};
use pyo3::{PyErr, PyResult};
use std::fmt::Display;

pub fn check_lower_bound<T>(val: T, min: Option<T>, instance_path: &InstancePath) -> PyResult<()>
where
    T: PartialOrd + Display,
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
where
    T: PartialOrd + Display,
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

pub fn check_min_length(
    val: &str,
    min: Option<usize>,
    instance_path: &InstancePath,
) -> PyResult<()> {
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

pub fn check_max_length(
    val: &str,
    max: Option<usize>,
    instance_path: &InstancePath,
) -> PyResult<()> {
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
    raise_error(
        format!(r#""{}" is a required property"#, property),
        instance_path,
    )
    .unwrap_err()
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
    ($type_: expr, $value: expr, $path: expr) => {{
        crate::validator::validators::_invalid_type($type_, $value, $path)?;
        unreachable!(); // todo: Discard the use of unreachable
    }};
}

pub fn _invalid_enum_item(
    items: EnumItems,
    value: Value,
    instance_path: &InstancePath,
) -> PyResult<()> {
    let error = match value.as_str() {
        Some(val) => format!(r#""{}" is not one of {}"#, val, items),
        None => format!(r#"{} is not one of {}"#, value.to_string()?, items),
    };
    raise_error(error, instance_path)?;
    Ok(())
}

macro_rules! invalid_enum_item {
    ($items: expr, $value: expr, $path: expr) => {{
        crate::validator::validators::_invalid_enum_item($items, $value, $path)?;
        unreachable!(); // todo: Discard the use of unreachable
    }};
}

pub(crate) use invalid_enum_item;
pub(crate) use invalid_type;
