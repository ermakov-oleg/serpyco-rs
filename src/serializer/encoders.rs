use crate::serializer::py::{create_new_object, decimal, py_len};
use pyo3::exceptions::PyException;
use pyo3::types::{PyDict, PyString, PyTuple};
use pyo3::{pyclass, pymethods, AsPyPointer, Py, PyAny, PyResult, Python, ToPyObject};
use pyo3_ffi::{PyObject, Py_ssize_t};
use std::fmt::Debug;

pyo3::create_exception!(serpyco_rs, ValidationError, PyException);

pub trait Encoder: Debug {
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject>;
    fn load(&self, value: &PyAny) -> PyResult<Py<PyAny>>;
}

#[pyclass]
#[derive(Debug)]
pub struct Serializer {
    pub encoder: Box<dyn Encoder + Send>,
}

#[pymethods]
impl Serializer {
    pub fn dump(&self, value: &PyAny) -> PyResult<Py<PyAny>> {
        unsafe {
            Ok(Py::from_owned_ptr(
                value.py(),
                self.encoder.dump(value.as_ptr())?,
            ))
        }
    }
    pub fn load(&self, value: &PyAny) -> PyResult<Py<PyAny>> {
        self.encoder.load(value)
    }
}

#[derive(Debug)]
pub struct NoopEncoder;

impl Encoder for NoopEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        Ok(value)
    }

    #[inline]
    fn load(&self, value: &PyAny) -> PyResult<Py<PyAny>> {
        Ok(value.into())
    }
}

#[derive(Debug)]
pub struct DecimalEncoder;

impl Encoder for DecimalEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        Ok(value)
    }

    #[inline]
    fn load(&self, value: &PyAny) -> PyResult<Py<PyAny>> {
        match decimal(value.py()).call1((value,)) {
            Ok(val) => Ok(Py::from(val)),
            Err(e) => Err(ValidationError::new_err(format!(
                "invalid Decimal value: {:?} error: {:?}",
                value, e
            ))),
        }
    }
}

#[derive(Debug)]
pub struct DictionaryEncoder {
    pub key_encoder: Box<dyn Encoder + Send>,
    pub value_encoder: Box<dyn Encoder + Send>,
}

impl Encoder for DictionaryEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        let dict_ptr = ffi!(PyDict_New());

        Python::with_gil(|py| {
            let value: Py<PyAny> = unsafe { Py::from_owned_ptr(py, value) };

            for i in value.call_method0(py, "items")?.as_ref(py).iter()? {
                let item = i?.downcast::<PyTuple>()?;
                let key = self.key_encoder.dump(item[0].as_ptr())?;
                let value = self.value_encoder.dump(item[1].as_ptr())?;

                ffi!(PyDict_SetItem(dict_ptr, key, value));
            }
            PyResult::Ok(())
        })?;

        Ok(dict_ptr)
    }

    #[inline]
    fn load(&self, value: &PyAny) -> PyResult<Py<PyAny>> {
        let result = PyDict::new(value.py());
        for i in value.call_method0("items")?.iter()? {
            let item = i?.downcast::<PyTuple>()?;
            let key = &item[0];
            let value = &item[1];
            result.set_item(self.key_encoder.load(key)?, self.value_encoder.load(value)?)?
        }

        Ok(result.into())
    }
}

#[derive(Debug)]
pub struct ArrayEncoder {
    pub encoder: Box<dyn Encoder + Send>,
}

impl Encoder for ArrayEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        let len = Python::with_gil(|py| {
            let value: Py<PyAny> = unsafe { Py::from_owned_ptr(py, value) };
            py_len(value.as_ref(py))
        })?;

        let list = ffi!(PyList_New(len));

        for i in 0..len {
            let item = ffi!(PyList_GetItem(value, i));
            let val = self.encoder.dump(item)?;

            ffi!(PyList_SetItem(list, i, val));
        }

        Ok(list)
    }

    #[inline]
    fn load(&self, value: &PyAny) -> PyResult<Py<PyAny>> {
        let iterable = value.iter()?;
        let len: Py_ssize_t = py_len(value)?;
        let mut result = Vec::with_capacity(len as usize);
        for i in iterable {
            result.push(self.encoder.load(i.unwrap())?);
        }
        Ok(result.to_object(value.py()))
    }
}

#[derive(Debug)]
pub struct EntityEncoder {
    pub(crate) create_new_object_args: Py<PyTuple>,
    pub(crate) fields: Vec<Field>,
}

#[derive(Debug)]
pub struct Field {
    pub(crate) name: Py<PyString>,
    pub(crate) dict_key: Py<PyString>,
    pub(crate) encoder: Box<dyn Encoder + Send>,
    pub(crate) default: Option<Py<PyAny>>,
    pub(crate) default_factory: Option<Py<PyAny>>,
}

impl Encoder for EntityEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        let dict_ptr = ffi!(PyDict_New());

        for field in &self.fields {
            let field_val = ffi!(PyObject_GetAttr(value, field.name.as_ptr()));
            let dump_result = field.encoder.dump(field_val)?;
            ffi!(PyDict_SetItem(
                dict_ptr,
                field.dict_key.as_ptr(),
                dump_result
            ));
        }

        Ok(dict_ptr)
    }

    #[inline]
    fn load(&self, value: &PyAny) -> PyResult<Py<PyAny>> {
        let py = value.py();
        let obj = create_new_object(&self.create_new_object_args.as_ref(py))?;
        for field in &self.fields {
            let val = match value.get_item(&field.dict_key) {
                Ok(val) => field.encoder.load(val)?,
                Err(e) => match (&field.default, &field.default_factory) {
                    (Some(val), _) => val.clone(),
                    (_, Some(val)) => val.call0(py)?,
                    (None, _) => {
                        return Err(ValidationError::new_err(format!(
                            "data dictionary is missing required parameter {} (err: {})",
                            &field.name, e
                        )))
                    }
                },
            };
            obj.setattr(field.name.as_ref(py), val)?;
        }
        Ok(Py::from(obj))
    }
}

macro_rules! ffi {
    ($fn:ident()) => {
        unsafe { pyo3_ffi::$fn() }
    };

    ($fn:ident($obj1:expr)) => {
        unsafe { pyo3_ffi::$fn($obj1) }
    };

    ($fn:ident($obj1:expr, $obj2:expr)) => {
        unsafe { pyo3_ffi::$fn($obj1, $obj2) }
    };

    ($fn:ident($obj1:expr, $obj2:expr, $obj3:expr)) => {
        unsafe { pyo3_ffi::$fn($obj1, $obj2, $obj3) }
    };

    ($fn:ident($obj1:expr, $obj2:expr, $obj3:expr, $obj4:expr)) => {
        unsafe { pyo3_ffi::$fn($obj1, $obj2, $obj3, $obj4) }
    };
}

pub(crate) use ffi;
