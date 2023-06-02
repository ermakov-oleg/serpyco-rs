mod serializer;

use pyo3::prelude::*;

#[pymodule]
fn _serpyco_rs(py: Python, m: &PyModule) -> PyResult<()> {
    serializer::init(py);
    m.add_class::<serializer::Serializer>()?;
    m.add(
        "ValidationError",
        py.get_type::<serializer::ValidationError>(),
    )?;

    Ok(())
}
