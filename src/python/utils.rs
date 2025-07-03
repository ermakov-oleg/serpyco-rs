use pyo3::prelude::PyAnyMethods;
use pyo3::types::PyString;
use pyo3::{Bound, PyAny};

pub fn fmt_py(value: &Bound<'_, PyAny>) -> String {
    match value.is_instance_of::<PyString>() {
        true => format!(r#""{value}""#),
        false => format!(r"{value}"),
    }
}
