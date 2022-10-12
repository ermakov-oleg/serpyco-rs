use crate::serializer::macros::ffi;
use crate::serializer::py::{create_new_object, from_ptr_or_err, iter_over_dict_items, obj_to_str, py_len, py_object_call1_make_tuple_or_err, py_object_get_attr, py_object_get_item, py_object_set_attr, py_tuple_get_item, to_decimal};
use crate::serializer::types::{NONE_PY_TYPE, UUID_PY_TYPE, VALUE_STR};
use pyo3::exceptions::PyException;
use pyo3::types::{PyString, PyTuple};
use pyo3::{pyclass, pymethods, AsPyPointer, Py, PyAny, PyResult, Python};
use pyo3_ffi::PyObject;
use std::fmt::Debug;

use super::macros::call_object;

pyo3::create_exception!(serpyco_rs, ValidationError, PyException);

pub trait Encoder: Debug {
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject>;
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject>;
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
            Ok(Py::from_borrowed_ptr(
                value.py(),
                self.encoder.dump(value.as_ptr())?,
            ))
        }
    }
    pub fn load(&self, value: &PyAny) -> PyResult<Py<PyAny>> {
        unsafe {
            Ok(Py::from_borrowed_ptr(
                value.py(),
                self.encoder.load(value.as_ptr())?,
            ))
        }
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
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        Ok(value)
    }
}

#[derive(Debug)]
pub struct DecimalEncoder;

impl Encoder for DecimalEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        obj_to_str(value)
    }

    #[inline]
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        to_decimal(value).map_err(|e| {
            ValidationError::new_err(format!("invalid Decimal value: {:?} error: {:?}", value, e))
        })
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

        for i in iter_over_dict_items(value)? {
            let item = i?;
            let key = self.key_encoder.dump(py_tuple_get_item(item, 0)?)?;
            let value = self.value_encoder.dump(py_tuple_get_item(item, 1)?)?;

            ffi!(PyDict_SetItem(dict_ptr, key, value));
        }

        Ok(dict_ptr)
    }

    #[inline]
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        let dict_ptr = ffi!(PyDict_New());

        for i in iter_over_dict_items(value)? {
            let item = i?;
            let key = self.key_encoder.load(py_tuple_get_item(item, 0)?)?;
            let value = self.value_encoder.load(py_tuple_get_item(item, 1)?)?;
            ffi!(PyDict_SetItem(dict_ptr, key, value));
        }

        Ok(dict_ptr)
    }
}

#[derive(Debug)]
pub struct ArrayEncoder {
    pub encoder: Box<dyn Encoder + Send>,
}

impl Encoder for ArrayEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        let len = py_len(value)?;

        let list = ffi!(PyList_New(len));

        for i in 0..len {
            let item = ffi!(PyList_GetItem(value, i));
            let val = self.encoder.dump(item)?;

            ffi!(PyList_SetItem(list, i, val));
        }

        Ok(list)
    }

    #[inline]
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        let len = py_len(value)?;
        let list = ffi!(PyList_New(len));
        for i in 0..len {
            let item = ffi!(PyList_GetItem(value, i));
            let val = self.encoder.load(item)?;
            ffi!(PyList_SetItem(list, i, val));
        }
        Ok(list)
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
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        Python::with_gil(|py| {
            let obj = create_new_object(&self.create_new_object_args.as_ref(py))?;
            for field in &self.fields {
                let val = match py_object_get_item(value, field.dict_key.as_ptr()) {
                    Ok(val) => field.encoder.load(val)?,
                    Err(e) => match (&field.default, &field.default_factory) {
                        (Some(val), _) => val.clone().as_ptr(),
                        (_, Some(val)) => {
                            call_object!(val.as_ptr())?
                        },
                        (None, _) => {
                            return Err(ValidationError::new_err(format!(
                                "data dictionary is missing required parameter {} (err: {})",
                                &field.name, e
                            )))
                        }
                    },
                };
                py_object_set_attr(obj, field.name.as_ptr(), val)?
            }
            Ok(obj)
        })
    }
}

#[derive(Debug)]
pub struct UUIDEncoder;

impl Encoder for UUIDEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        obj_to_str(value)
    }

    #[inline]
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        py_object_call1_make_tuple_or_err(unsafe { UUID_PY_TYPE }, value)
    }
}

#[derive(Debug)]
pub struct EnumEncoder {
    pub(crate) enum_type: pyo3::PyObject,
}

impl Encoder for EnumEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        obj_to_str(py_object_get_attr(value, unsafe { VALUE_STR })?)
    }

    #[inline]
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        py_object_call1_make_tuple_or_err(self.enum_type.as_ptr(), value)
    }
}

#[derive(Debug)]
pub struct OptionalEncoder {
    pub(crate) encoder: Box<dyn Encoder + Send>,
}

impl Encoder for OptionalEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        if value == unsafe {NONE_PY_TYPE} {
            Ok(value)
        } else {
            self.encoder.dump(value)
        }
    }

    #[inline]
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        if value == unsafe {NONE_PY_TYPE} {
            Ok(value)
        } else {
            self.encoder.load(value)
        }
    }
}
