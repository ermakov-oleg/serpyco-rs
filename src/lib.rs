// Benchmark-only experiment: a fully specialized JSON codec for the
// `bench/compare/github_issue` model. Off by default; built with
// `maturin develop --release --features bench-codec`.
#[cfg(feature = "bench-codec")]
mod bench_codec;
mod errors;
mod format;
mod python;
mod serde_error;
mod serializer;
mod validator;

use pyo3::prelude::*;

#[pymodule(gil_used = false)]
fn _serpyco_rs(py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<serializer::Serializer>()?;
    #[cfg(feature = "bench-codec")]
    m.add_class::<bench_codec::GithubIssueCodec>()?;

    // Errors
    m.add("ValidationError", py.get_type::<errors::ValidationError>())?;
    m.add(
        "SchemaValidationError",
        py.get_type::<errors::SchemaValidationError>(),
    )?;
    m.add("ErrorItem", py.get_type::<errors::ErrorItem>())?;
    m.add("DecodeError", py.get_type::<errors::DecodeError>())?;
    Ok(())
}
