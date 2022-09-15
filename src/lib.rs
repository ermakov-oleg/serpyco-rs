mod serializer;

use pyo3::prelude::*;

/// A Python module implemented in Rust.
#[pymodule]
fn serpyco_rs(_py: Python, m: &PyModule) -> PyResult<()> {
    serializer::init();
    m.add_function(wrap_pyfunction!(serializer::make_serializer, m)?)?;
    Ok(())
}
