mod serializer;

use pyo3::prelude::*;

/// Formats the sum of two numbers as string.
#[pyfunction]
fn sum_as_string(a: usize, b: usize) -> PyResult<String> {
    Ok((a + b).to_string())
}


//
// fn serialize(obj: &PyAny) -> PyResult<String> {
//
// }


/// A Python module implemented in Rust.
#[pymodule]
fn serpyco_rs(_py: Python, m: &PyModule) -> PyResult<()> {
    serializer::init();
    m.add_function(wrap_pyfunction!(sum_as_string, m)?)?;
    m.add_function(wrap_pyfunction!(serializer::make_serializer, m)?)?;
    Ok(())
}