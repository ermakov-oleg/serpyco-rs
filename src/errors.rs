use pyo3::pyclass::CompareOp;
use pyo3::types::PyList;
use pyo3::{exceptions, pyclass, pymethods, Py, PyCell, PyErr, PyErrArguments, PyRef, PyTypeInfo};
use std::fmt::Debug;

#[pyclass(extends=exceptions::PyValueError, module="serpyco_rs", subclass)]
#[derive(Debug)]
pub(crate) struct ValidationError {
    #[pyo3(get)]
    message: String,
}

#[pymethods]
impl ValidationError {
    #[new]
    fn new(message: String) -> Self {
        ValidationError { message }
    }
    fn __str__(&self) -> String {
        self.message.clone()
    }
    fn __repr__(&self) -> String {
        format!("<ValidationError: '{}'>", self.message)
    }
}

#[pyclass(extends=ValidationError, module="serpyco_rs")]
#[derive(Debug)]
pub(crate) struct SchemaValidationError {
    #[pyo3(get)]
    errors: Py<PyList>,
}

#[pymethods]
impl SchemaValidationError {
    #[new]
    pub(crate) fn new(message: String, errors: Py<PyList>) -> (Self, ValidationError) {
        (
            SchemaValidationError { errors },
            ValidationError::new(message),
        )
    }

    fn __str__(self_: PyRef<'_, Self>) -> String {
        let super_ = self_.as_ref(); // Get &ValidationError

        let mut result = String::new();
        result.push_str(&format!("{}:\n", super_.message));

        for error in self_.errors.as_ref(self_.py()).iter() {
            let message = match error.downcast::<PyCell<ErrorItem>>() {
                Ok(cell) => cell.borrow().__str__(),
                Err(e) => format!("Error: {}", e),
            };
            result.push_str(&format!("- {}\n", message));
        }
        result
    }

    fn __repr__(self_: PyRef<'_, Self>) -> String {
        let super_ = self_.as_ref(); // Get &ValidationError

        let mut result = String::new();
        result.push_str("SchemaValidationError(\n");
        result.push_str(&format!("    message=\"{}\",\n", super_.message));
        result.push_str("    errors=[\n");
        for error in self_.errors.as_ref(self_.py()).iter() {
            let message = match error.downcast::<PyCell<ErrorItem>>() {
                Ok(cell) => cell.borrow().__repr__(),
                Err(e) => format!("Error: {}", e),
            };
            result.push_str(&format!("        {},\n", message));
        }
        result.push_str("    ]\n)");
        result
    }
}

#[pyclass(frozen, module = "serpyco_rs")]
#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ErrorItem {
    #[pyo3(get)]
    message: String,
    #[pyo3(get)]
    schema_path: String,
    #[pyo3(get)]
    instance_path: String,
}

#[pymethods]
impl ErrorItem {
    #[new]
    pub fn new(message: String, schema_path: String, instance_path: String) -> Self {
        ErrorItem {
            message,
            schema_path,
            instance_path,
        }
    }

    fn __str__(&self) -> String {
        format!(
            "{} (schema_path='{}', instance_path='{}')",
            self.message, self.schema_path, self.instance_path
        )
    }
    fn __repr__(&self) -> String {
        format!(
            "ErrorItem(message='{}', schema_path='{}', instance_path='{}')",
            self.message, self.schema_path, self.instance_path
        )
    }
    fn __richcmp__(&self, other: &ErrorItem, op: CompareOp) -> bool {
        op.matches(self.cmp(other))
    }
}

pub(crate) trait ToPyErr {
    #[inline]
    fn new_err<A>(args: A) -> PyErr
    where
        A: PyErrArguments + Debug + 'static,
        Self: PyTypeInfo,
    {
        PyErr::new::<Self, A>(args)
    }
}

impl ToPyErr for ValidationError {}
impl ToPyErr for SchemaValidationError {}
