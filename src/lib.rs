mod errors;
mod jsonschema;
mod python;
mod serializer;

use pyo3::prelude::*;

#[pymodule]
fn _serpyco_rs(py: Python, m: &PyModule) -> PyResult<()> {
    python::init(py);
    jsonschema::init();
    m.add_class::<serializer::Serializer>()?;
    m.add("ValidationError", py.get_type::<errors::ValidationError>())?;
    m.add(
        "SchemaValidationError",
        py.get_type::<errors::SchemaValidationError>(),
    )?;
    m.add("ErrorItem", py.get_type::<errors::ErrorItem>())?;
    Ok(())
}
