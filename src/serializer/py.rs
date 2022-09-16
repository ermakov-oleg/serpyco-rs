use once_cell::sync::OnceCell;
use pyo3::types::{PyLong, PyTuple};
use pyo3::{Py, PyAny, PyErr, PyObject, PyResult, Python};

static DECIMAL: OnceCell<PyObject> = OnceCell::new();
static BUILTINS: OnceCell<PyObject> = OnceCell::new();
static PY_LEN: OnceCell<PyObject> = OnceCell::new();
static NOT_SET: OnceCell<PyObject> = OnceCell::new();
static OBJECT_NEW: OnceCell<PyObject> = OnceCell::new();

pub fn decimal(py: Python) -> PyResult<&PyAny> {
    DECIMAL
        .get_or_try_init(|| Ok(py.import("decimal")?.getattr("Decimal")?.into()))
        .map(|o| o.as_ref(py))
}

fn builtins(py: Python) -> PyResult<&PyAny> {
    BUILTINS
        .get_or_try_init(|| Ok(py.import("builtins")?.into()))
        .map(|o| o.as_ref(py))
}

pub fn py_len(obj: &PyAny) -> PyResult<&PyLong> {
    let py = obj.py();
    let len = PY_LEN
        .get_or_try_init(|| {
            let builtins = builtins(py)?;
            Ok::<Py<PyAny>, PyErr>(builtins.getattr("len")?.into())
        })
        .map(|o| o.as_ref(py))?;
    Ok(len.call1((obj,))?.downcast()?)
}

pub fn is_not_set(obj: &PyAny) -> PyResult<bool> {
    let py = obj.py();
    let not_set = NOT_SET
        .get_or_try_init(|| {
            Ok::<Py<PyAny>, PyErr>(
                py.import("serpyco_rs")?
                    .getattr("_describe")?
                    .getattr("NOT_SET")?
                    .into(),
            )
        })
        .map(|o| o.as_ref(py))?;
    Ok(not_set.is(obj))
}

pub fn create_new_object(cls: &PyTuple) -> PyResult<&PyAny> {
    let py = cls.py();
    let __new__ = OBJECT_NEW
        .get_or_try_init(|| {
            let builtins = builtins(py)?;
            Ok::<Py<PyAny>, PyErr>(builtins.getattr("object")?.getattr("__new__")?.into())
        })
        .map(|o| o.as_ref(py))?;
    __new__.call1(cls)
}
