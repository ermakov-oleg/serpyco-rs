use pyo3::once_cell::GILOnceCell;
use pyo3::types::{PyLong, PyTuple};
use pyo3::{Py, PyAny, PyErr, PyObject, PyResult, Python};
use pyo3_ffi::Py_ssize_t;

static DECIMAL: GILOnceCell<PyObject> = GILOnceCell::new();
static BUILTINS: GILOnceCell<PyObject> = GILOnceCell::new();
static PY_LEN: GILOnceCell<PyObject> = GILOnceCell::new();
static NOT_SET: GILOnceCell<PyObject> = GILOnceCell::new();
static OBJECT_NEW: GILOnceCell<PyObject> = GILOnceCell::new();

pub fn decimal(py: Python) -> &PyAny {
    DECIMAL
        .get_or_init(py, || {
            py.import("decimal")
                .expect("Error when importing decimal.Decimal")
                .getattr("Decimal")
                .expect("Error when importing decimal.Decimal")
                .into()
        })
        .as_ref(py)
}

fn builtins(py: Python) -> &PyAny {
    BUILTINS
        .get_or_init(py, || {
            py.import("builtins")
                .expect("Error when importing builtins")
                .into()
        })
        .as_ref(py)
}

pub fn py_len(obj: &PyAny) -> PyResult<Py_ssize_t> {
    let py = obj.py();
    let len = PY_LEN
        .get_or_init(py, || {
            let builtins = builtins(py);
            builtins
                .getattr("len")
                .expect("Error when importing builtins.len")
                .into()
        })
        .as_ref(py);
    Ok(len.call1((obj,))?.extract()?)
}

pub fn is_not_set(obj: &PyAny) -> PyResult<bool> {
    let py = obj.py();
    let not_set = NOT_SET
        .get_or_init(py, || {
            py.import("serpyco_rs")
                .expect("Error when importing serpyco_rs")
                .getattr("_describe")
                .expect("Error when importing serpyco_rs._describe")
                .getattr("NOT_SET")
                .expect("Error when importing serpyco_rs._describe.NOT_SET")
                .into()
        })
        .as_ref(py);
    Ok(not_set.is(obj))
}

pub fn create_new_object(cls: &PyTuple) -> PyResult<&PyAny> {
    let py = cls.py();
    let __new__ = OBJECT_NEW
        .get_or_init(py, || {
            let builtins = builtins(py);
            builtins
                .getattr("object")
                .expect("Error when importing builtins.object")
                .getattr("__new__")
                .expect("Error getattr builtins.object.__new__")
                .into()
        })
        .as_ref(py);
    __new__.call1(cls)
}
