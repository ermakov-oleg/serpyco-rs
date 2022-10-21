mod serializer;

use pyo3::prelude::*;

/// A Python module implemented in Rust.
#[pymodule]
fn _serpyco_rs(py: Python, m: &PyModule) -> PyResult<()> {
    serializer::init(py);
    m.add_class::<serializer::Serializer>()?;
    m.add_function(wrap_pyfunction!(serializer::make_encoder, m)?)?;
    m.add(
        "ValidationError",
        py.get_type::<serializer::ValidationError>(),
    )?;

    Ok(())
}
