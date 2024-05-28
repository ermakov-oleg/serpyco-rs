use crate::validator::{raise_error, InstancePath};

use pyo3::prelude::PyAnyMethods;
use pyo3::types::{PySequence, PyString};
use pyo3::{Bound, PyAny, PyErr, PyResult};
use std::cmp::Ordering;
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

pub fn _check_bounds<T>(
    val: T,
    min: Option<T>,
    max: Option<T>,
    instance_path: &InstancePath,
) -> PyResult<()>
where
    T: PartialOrd + Display + Copy,
{
    if min.is_none() && max.is_none() {
        return Ok(());
    }
    check_lower_bound(val, min, instance_path)?;
    check_upper_bound(val, max, instance_path)?;
    Ok(())
}

macro_rules! check_bounds {
    ($val: expr, $type_info: expr, $path: expr) => {
        crate::validator::validators::_check_bounds($val, $type_info.min, $type_info.max, $path)
    };
}

pub fn check_min_length(
    val: &Bound<'_, PyString>,
    len: usize,
    min: Option<usize>,
    instance_path: &InstancePath,
) -> PyResult<()> {
    if let Some(min) = min {
        if len <= min {
            raise_error(
                format!(r#""{}" is shorter than {} characters"#, val, min),
                instance_path,
            )?;
        }
    }
    Ok(())
}

pub fn check_max_length(
    val: &Bound<'_, PyString>,
    len: usize,
    max: Option<usize>,
    instance_path: &InstancePath,
) -> PyResult<()> {
    if let Some(max) = max {
        if len > max {
            raise_error(
                format!(r#""{}" is longer than {} characters"#, val, max),
                instance_path,
            )?;
        }
    }
    Ok(())
}

pub fn check_length(
    val: &Bound<'_, PyString>,
    min: Option<usize>,
    max: Option<usize>,
    instance_path: &InstancePath,
) -> PyResult<()> {
    if min.is_none() && max.is_none() {
        return Ok(());
    }
    let len = val.len()?;
    check_min_length(val, len, min, instance_path)?;
    check_max_length(val, len, max, instance_path)?;
    Ok(())
}

#[cold]
pub fn missing_required_property(property: &str, instance_path: &InstancePath) -> PyErr {
    let instance_path = instance_path.push(property);
    raise_error(
        format!(r#""{}" is a required property"#, property),
        &instance_path,
    )
    .unwrap_err()
}

pub fn check_sequence_size(
    val: &Bound<'_, PySequence>,
    seq_len: usize,
    size: usize,
    instance_path: Option<&InstancePath>,
) -> PyResult<()> {
    match seq_len.cmp(&size) {
        Ordering::Equal => Ok(()),
        Ordering::Less => {
            let instance_path = instance_path.cloned().unwrap_or(InstancePath::new());
            raise_error(
                format!(r#"{} has less than {} items"#, val, size),
                &instance_path,
            )
        }
        Ordering::Greater => {
            let instance_path = instance_path.cloned().unwrap_or(InstancePath::new());
            raise_error(
                format!(r#"{} has more than {} items"#, val, size),
                &instance_path,
            )
        }
    }
}

pub fn no_encoder_for_discriminator<K, D>(
    key: &K,
    discriminators: &[D],
    instance_path: &InstancePath,
) -> PyErr
where
    K: Display,
    D: Display,
{
    let items = discriminators
        .iter()
        .map(|s| format!(r#""{}""#, s))
        .collect::<Vec<_>>()
        .join(", ");
    raise_error(
        format!(
            r#""{}" is not one of [{}] discriminator values"#,
            key, items
        ),
        instance_path,
    )
    .unwrap_err()
}

pub fn _invalid_type_new(
    type_: &str,
    value: &Bound<'_, PyAny>,
    instance_path: &InstancePath,
) -> PyResult<()> {
    let error = format!(r#"{} is not of type "{}""#, fmt_py(value), type_);
    raise_error(error, instance_path)?;
    Ok(())
}

macro_rules! invalid_type {
    ($type_: expr, $value: expr, $path: expr) => {{
        crate::validator::validators::_invalid_type_new($type_, $value, $path)?;
        unreachable!(); // todo: Discard the use of unreachable
    }};
}

macro_rules! invalid_type_dump {
    ($type_: expr, $value: expr) => {{
        let error = format!(r#""{}" is not of type "{}""#, $value.to_string(), $type_);
        let instance_path = InstancePath::new();
        crate::validator::raise_error(error, &instance_path)?;
        unreachable!(); // todo: Discard the use of unreachable
    }};
}

pub fn _invalid_enum_item(
    items: &str,
    value: &Bound<'_, PyAny>,
    instance_path: &InstancePath,
) -> PyResult<()> {
    let error = format!(r#"{} is not one of {}"#, fmt_py(value), items);
    raise_error(error, instance_path)?;
    Ok(())
}

macro_rules! invalid_enum_item {
    ($items: expr, $value: expr, $path: expr) => {{
        crate::validator::validators::_invalid_enum_item($items, $value, $path)?;
        unreachable!(); // todo: Discard the use of unreachable
    }};
}

pub fn str_as_bool(str: &str) -> Option<bool> {
    match str.as_bytes() {
        [b't'] | [b'T'] | b"true" | b"True" | b"TRUE" => Some(true),
        [b'f'] | [b'F'] | b"false" | b"False" | b"FALSE" => Some(false),
        _ => None,
    }
}

use crate::python::fmt_py;
pub(crate) use check_bounds;
pub(crate) use invalid_enum_item;
pub(crate) use invalid_type;
pub(crate) use invalid_type_dump;
