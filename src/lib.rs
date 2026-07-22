mod errors;
mod python;
mod serde_error;
mod serializer;
mod validator;

use pyo3::prelude::*;

#[pymodule(gil_used = false)]
fn _serpyco_rs(py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<serializer::Serializer>()?;

    // Errors
    m.add("ValidationError", py.get_type::<errors::ValidationError>())?;
    m.add(
        "SchemaValidationError",
        py.get_type::<errors::SchemaValidationError>(),
    )?;
    m.add("ErrorItem", py.get_type::<errors::ErrorItem>())?;
    Ok(())
}
