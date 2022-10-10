use crate::serializer::macros::ffi;
use crate::serializer::types::ITEMS_STR;
use pyo3::once_cell::GILOnceCell;
use pyo3::types::PyTuple;
use pyo3::{ffi, AsPyPointer, Py, PyAny, PyErr, PyObject, PyResult, Python, ToPyObject};
use pyo3_ffi::Py_ssize_t;
use std::os::raw::{c_char, c_int};
use std::ptr::NonNull;

static DECIMAL: GILOnceCell<PyObject> = GILOnceCell::new();
static BUILTINS: GILOnceCell<PyObject> = GILOnceCell::new();
static NOT_SET: GILOnceCell<PyObject> = GILOnceCell::new();
static OBJECT_NEW: GILOnceCell<PyObject> = GILOnceCell::new();

fn _decimal_cls() -> Py<PyAny> {
    Python::with_gil(|py| {
        DECIMAL
            .get_or_init(py, || {
                py.import("decimal")
                    .expect("Error when importing decimal.Decimal")
                    .getattr("Decimal")
                    .expect("Error when importing decimal.Decimal")
                    .to_object(py)
            })
            .to_object(py)
    })
}

pub fn to_decimal(value: *mut ffi::PyObject) -> PyResult<*mut ffi::PyObject> {
    let decimal = _decimal_cls();
    py_object_call1_make_tuple_or_err(decimal.as_ptr(), value)
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

pub fn py_len(obj: *mut ffi::PyObject) -> PyResult<Py_ssize_t> {
    let v = ffi!(PyObject_Size(obj));
    if v == -1 {
        Err(Python::with_gil(|py| PyErr::fetch(py)))
    } else {
        Ok(v)
    }
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

pub fn create_new_object(cls: &PyTuple) -> PyResult<*mut ffi::PyObject> {
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
    py_object_call1_or_err(__new__.as_ptr(), cls.as_ptr())
}

pub fn iter_over_dict_items(obj: *mut ffi::PyObject) -> PyResult<PyObjectIterator> {
    let items = py_object_callmethod_noargs(obj, unsafe { ITEMS_STR })?;
    to_iter(items)
}

pub fn to_py_string(s: &str) -> *mut ffi::PyObject {
    ffi!(PyUnicode_InternFromString(s.as_ptr() as *const c_char))
}

fn py_object_call1_or_err(
    obj: *mut ffi::PyObject,
    args: *mut ffi::PyObject,
) -> PyResult<*mut ffi::PyObject> {
    from_ptr_or_err(ffi!(PyObject_CallObject(obj, args)))
}

fn py_object_callmethod_noargs(
    obj: *mut ffi::PyObject,
    name: *mut ffi::PyObject,
) -> PyResult<*mut ffi::PyObject> {
    from_ptr_or_err(ffi!(PyObject_CallMethodNoArgs(obj, name)))
}

fn py_object_call1_make_tuple_or_err(
    obj: *mut ffi::PyObject,
    arg: *mut ffi::PyObject,
) -> PyResult<*mut ffi::PyObject> {
    let tuple_arg = from_ptr_or_err(ffi!(PyTuple_Pack(1, arg)))?;
    let result = py_object_call1_or_err(obj, tuple_arg)?;
    ffi!(Py_DECREF(tuple_arg));
    Ok(result)
}

pub fn py_object_set_attr(
    obj: *mut ffi::PyObject,
    attr_name: *mut ffi::PyObject,
    value: *mut ffi::PyObject,
) -> PyResult<()> {
    let ret = ffi!(PyObject_SetAttr(obj, attr_name, value));
    error_on_minusone(ret)
}

pub fn py_tuple_get_item(obj: *mut ffi::PyObject, index: usize) -> PyResult<*mut ffi::PyObject> {
    from_ptr_or_err(ffi!(PyTuple_GetItem(obj, index as Py_ssize_t)))
}

pub fn py_object_get_item(
    obj: *mut ffi::PyObject,
    key: *mut ffi::PyObject,
) -> PyResult<*mut ffi::PyObject> {
    from_ptr_or_err(ffi!(PyObject_GetItem(obj, key)))
}

fn from_ptr_or_opt(ptr: *mut ffi::PyObject) -> Option<*mut ffi::PyObject> {
    NonNull::new(ptr).map(|p| p.as_ptr())
}

fn from_ptr_or_err(ptr: *mut ffi::PyObject) -> PyResult<*mut ffi::PyObject> {
    from_ptr_or_opt(ptr).ok_or_else(|| Python::with_gil(|py| PyErr::fetch(py)))
}

#[inline]
pub fn error_on_minusone(result: c_int) -> PyResult<()> {
    if result != -1 {
        Ok(())
    } else {
        Err(Python::with_gil(|py| PyErr::fetch(py)))
    }
}

fn to_iter(obj: *mut ffi::PyObject) -> PyResult<PyObjectIterator> {
    let internal = PyObjectIterator(from_ptr_or_err(ffi!(PyObject_GetIter(obj)))?);
    Ok(internal)
}

pub struct PyObjectIterator(*mut ffi::PyObject);

impl Iterator for PyObjectIterator {
    type Item = PyResult<*mut ffi::PyObject>;

    fn next(&mut self) -> Option<Self::Item> {
        match from_ptr_or_opt(ffi!(PyIter_Next(self.0))) {
            Some(item) => Some(Ok(item)),
            None => Python::with_gil(|py| PyErr::take(py).map(Err)),
        }
    }
}
