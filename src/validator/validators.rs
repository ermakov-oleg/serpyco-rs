use crate::python::fmt_py;
use crate::serde_error::{SchemaError, SerdeError};
use crate::validator::InstancePath;

use pyo3::prelude::PyAnyMethods;
use pyo3::types::{PyList, PySequence, PyString};
use pyo3::{Bound, PyAny};
use std::cmp::Ordering;
use std::fmt::Display;

pub fn check_lower_bound<T>(
    val: T,
    min: Option<T>,
    inclusive: bool,
    instance_path: &InstancePath,
) -> Result<(), SerdeError>
where
    T: PartialOrd + Display,
{
    if let Some(min) = min {
        let violates = if inclusive { val < min } else { val <= min };
        if violates {
            return Err(SchemaError::new(
                format!("{val} is less than the minimum of {min}"),
                instance_path,
            )
            .into());
        }
    }
    Ok(())
}

pub fn check_upper_bound<T>(
    val: T,
    max: Option<T>,
    inclusive: bool,
    instance_path: &InstancePath,
) -> Result<(), SerdeError>
where
    T: PartialOrd + Display,
{
    if let Some(max) = max {
        let violates = if inclusive { val > max } else { val >= max };
        if violates {
            return Err(SchemaError::new(
                format!("{val} is greater than the maximum of {max}"),
                instance_path,
            )
            .into());
        }
    }
    Ok(())
}

pub fn _check_bounds<T>(
    val: T,
    min: Option<T>,
    max: Option<T>,
    inclusive_min: bool,
    inclusive_max: bool,
    instance_path: &InstancePath,
) -> Result<(), SerdeError>
where
    T: PartialOrd + Display + Copy,
{
    if min.is_none() && max.is_none() {
        return Ok(());
    }
    check_lower_bound(val, min, inclusive_min, instance_path)?;
    check_upper_bound(val, max, inclusive_max, instance_path)?;
    Ok(())
}

macro_rules! check_bounds {
    ($val: expr, $type_info: expr, $path: expr) => {
        crate::validator::validators::_check_bounds(
            $val,
            $type_info.min,
            $type_info.max,
            $type_info.inclusive_min,
            $type_info.inclusive_max,
            $path,
        )
    };
}

pub fn check_min_length(
    val: &Bound<'_, PyString>,
    len: usize,
    min: Option<usize>,
    instance_path: &InstancePath,
) -> Result<(), SerdeError> {
    if let Some(min) = min {
        if len < min {
            return Err(SchemaError::new(
                format!(r#""{val}" is shorter than {min} characters"#),
                instance_path,
            )
            .into());
        }
    }
    Ok(())
}

pub fn check_max_length(
    val: &Bound<'_, PyString>,
    len: usize,
    max: Option<usize>,
    instance_path: &InstancePath,
) -> Result<(), SerdeError> {
    if let Some(max) = max {
        if len > max {
            return Err(SchemaError::new(
                format!(r#""{val}" is longer than {max} characters"#),
                instance_path,
            )
            .into());
        }
    }
    Ok(())
}

pub fn check_length(
    val: &Bound<'_, PyString>,
    min: Option<usize>,
    max: Option<usize>,
    instance_path: &InstancePath,
) -> Result<(), SerdeError> {
    if min.is_none() && max.is_none() {
        return Ok(());
    }
    let len = val.len()?;
    check_min_length(val, len, min, instance_path)?;
    check_max_length(val, len, max, instance_path)?;
    Ok(())
}

#[cold]
pub fn missing_required_property(property: &str, instance_path: &InstancePath) -> SerdeError {
    let instance_path = instance_path.push(property);
    SchemaError::new(
        format!(r#""{property}" is a required property"#),
        &instance_path,
    )
    .into()
}

pub fn check_sequence_size(
    val: &Bound<'_, PySequence>,
    seq_len: usize,
    size: usize,
    instance_path: Option<&InstancePath>,
) -> Result<(), SerdeError> {
    match seq_len.cmp(&size) {
        Ordering::Equal => Ok(()),
        Ordering::Less => {
            let path = instance_path.cloned().unwrap_or_else(InstancePath::new);
            Err(SchemaError::new(format!(r#"{val} has less than {size} items"#), &path).into())
        }
        Ordering::Greater => {
            let path = instance_path.cloned().unwrap_or_else(InstancePath::new);
            Err(SchemaError::new(format!(r#"{val} has more than {size} items"#), &path).into())
        }
    }
}

pub fn check_sequence_bounds(
    val: &Bound<'_, PyList>,
    seq_len: usize,
    min: Option<usize>,
    max: Option<usize>,
    instance_path: Option<&InstancePath>,
) -> Result<(), SerdeError> {
    if min.is_none() && max.is_none() {
        return Ok(());
    }
    if let Some(min) = min {
        if seq_len < min {
            let path = instance_path.cloned().unwrap_or_else(InstancePath::new);
            return Err(
                SchemaError::new(format!(r#"{val} has less than {min} items"#), &path).into(),
            );
        }
    }
    if let Some(max) = max {
        if seq_len > max {
            let path = instance_path.cloned().unwrap_or_else(InstancePath::new);
            return Err(
                SchemaError::new(format!(r#"{val} has more than {max} items"#), &path).into(),
            );
        }
    }
    Ok(())
}

pub fn no_encoder_for_discriminator<K, D>(
    key: &K,
    discriminators: &[D],
    instance_path: &InstancePath,
) -> SerdeError
where
    K: Display,
    D: Display,
{
    let items = discriminators
        .iter()
        .map(|s| format!(r#""{s}""#))
        .collect::<Vec<_>>()
        .join(", ");
    SchemaError::new(
        format!(r#""{key}" is not one of [{items}] discriminator values"#),
        instance_path,
    )
    .into()
}

pub fn invalid_type_err(
    type_: &str,
    value: &Bound<'_, PyAny>,
    instance_path: &InstancePath,
) -> SerdeError {
    SchemaError::new(
        format!(r#"{} is not of type "{}""#, fmt_py(value), type_),
        instance_path,
    )
    .into()
}

macro_rules! invalid_type {
    ($type_: expr, $value: expr, $path: expr) => {
        return Err(crate::validator::validators::invalid_type_err(
            $type_, $value, $path,
        ))
    };
}

pub fn invalid_type_dump_err(type_: &str, value: &Bound<'_, PyAny>) -> SerdeError {
    SchemaError::new(
        format!(r#""{value}" is not of type "{type_}""#),
        &InstancePath::new(),
    )
    .into()
}

macro_rules! invalid_type_dump {
    ($type_: expr, $value: expr) => {
        return Err(crate::validator::validators::invalid_type_dump_err(
            $type_, $value,
        ))
    };
}

pub fn invalid_enum_item_err(
    items: &str,
    value: &Bound<'_, PyAny>,
    instance_path: &InstancePath,
) -> SerdeError {
    SchemaError::new(
        format!(r#"{} is not one of {}"#, fmt_py(value), items),
        instance_path,
    )
    .into()
}

macro_rules! invalid_enum_item {
    ($items: expr, $value: expr, $path: expr) => {
        return Err(crate::validator::validators::invalid_enum_item_err(
            $items, $value, $path,
        ))
    };
}

pub fn str_as_bool(str: &str) -> Option<bool> {
    match str.as_bytes() {
        [b't' | b'T'] | b"true" | b"True" | b"TRUE" => Some(true),
        [b'f' | b'F'] | b"false" | b"False" | b"FALSE" => Some(false),
        _ => None,
    }
}

pub(crate) use check_bounds;
pub(crate) use invalid_enum_item;
pub(crate) use invalid_type;
pub(crate) use invalid_type_dump;
