use crate::serializer::dateutil::{parse_date, parse_time};
use crate::serializer::py::{
    create_new_object, from_ptr_or_err, iter_over_dict_items, obj_to_str, py_len,
    py_object_call1_make_tuple_or_err, py_object_get_attr, py_object_get_item, py_object_set_attr,
    py_str_to_str, py_tuple_get_item, to_decimal,
};
use crate::serializer::types::{ISOFORMAT_STR, NONE_PY_TYPE, UUID_PY_TYPE, VALUE_STR};
use atomic_refcell::AtomicRefCell;
use pyo3::exceptions::{PyException, PyRuntimeError};
use pyo3::types::PyString;
use pyo3::{AsPyPointer, Py, PyAny, PyResult};
use pyo3_ffi::PyObject;
use std::fmt::Debug;
use std::sync::Arc;

use super::dateutil::parse_datetime;
use super::macros::{call_method, call_object, ffi};

use dyn_clone::{clone_trait_object, DynClone};

pyo3::create_exception!(serpyco_rs, ValidationError, PyException);

pub type TEncoder = dyn Encoder + Send + Sync;

pub trait Encoder: DynClone + Debug {
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject>;
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject>;
}

clone_trait_object!(Encoder);

#[derive(Debug, Clone)]
pub struct NoopEncoder;

impl Encoder for NoopEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        ffi!(Py_INCREF(value));
        Ok(value)
    }

    #[inline]
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        ffi!(Py_INCREF(value));
        Ok(value)
    }
}

#[derive(Debug, Clone)]
pub struct DecimalEncoder;

impl Encoder for DecimalEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        obj_to_str(value)
    }

    #[inline]
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        to_decimal(value).map_err(|e| {
            ValidationError::new_err(format!("invalid Decimal value: {value:?} error: {e:?}"))
        })
    }
}

#[derive(Debug, Clone)]
pub struct DictionaryEncoder {
    pub key_encoder: Box<TEncoder>,
    pub value_encoder: Box<TEncoder>,
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

#[derive(Debug, Clone)]
pub struct ArrayEncoder {
    pub encoder: Box<TEncoder>,
}

impl Encoder for ArrayEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        let len = py_len(value)?;

        let list = ffi!(PyList_New(len));

        for i in 0..len {
            let item = ffi!(PyList_GetItem(value, i)); // rc not changed
            let val = self.encoder.dump(item)?; // new obj or RC +1
            ffi!(PyList_SetItem(list, i, val)); // rc not changed
        }

        Ok(list)
    }

    #[inline]
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        let len = py_len(value)?;
        let list = ffi!(PyList_New(len));
        for i in 0..len {
            let item = ffi!(PyList_GetItem(value, i)); // rc not changed
            let val = self.encoder.load(item)?; // new obj or RC +1
            ffi!(PyList_SetItem(list, i, val)); // rc not changed
        }
        Ok(list)
    }
}

#[derive(Debug, Clone)]
pub struct EntityEncoder {
    pub(crate) cls: Py<PyAny>,
    pub(crate) fields: Vec<Field>,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub(crate) name: Py<PyString>,
    pub(crate) dict_key: Py<PyString>,
    pub(crate) encoder: Box<TEncoder>,
    pub(crate) default: Option<Py<PyAny>>,
    pub(crate) default_factory: Option<Py<PyAny>>,
}

impl Encoder for EntityEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        let dict_ptr = ffi!(PyDict_New());

        for field in &self.fields {
            let field_val = ffi!(PyObject_GetAttr(value, field.name.as_ptr())); // val RC +1
            let dump_result = field.encoder.dump(field_val)?; // new obj or RC +1

            ffi!(PyDict_SetItem(
                dict_ptr,
                field.dict_key.as_ptr(),
                dump_result
            )); // key and val RC +1
            ffi!(Py_DECREF(field_val));
            ffi!(Py_DECREF(dump_result));
        }

        Ok(dict_ptr)
    }

    #[inline]
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        let obj = create_new_object(self.cls.as_ptr())?;
        for field in &self.fields {
            let val = match py_object_get_item(value, field.dict_key.as_ptr()) {
                Ok(val) => field.encoder.load(val)?, // new obj or RC +1
                Err(e) => match (&field.default, &field.default_factory) {
                    (Some(val), _) => val.clone().as_ptr(),
                    (_, Some(val)) => call_object!(val.as_ptr())?,
                    (None, _) => {
                        return Err(ValidationError::new_err(format!(
                            "data dictionary is missing required parameter {} (err: {})",
                            &field.name, e
                        )))
                    }
                },
            };
            py_object_set_attr(obj, field.name.as_ptr(), val)?; // val RC +1
            ffi!(Py_DECREF(val));
        }
        Ok(obj)
    }
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct OptionalEncoder {
    pub(crate) encoder: Box<TEncoder>,
}

impl Encoder for OptionalEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        if value == unsafe { NONE_PY_TYPE } {
            ffi!(Py_INCREF(NONE_PY_TYPE));
            Ok(value)
        } else {
            self.encoder.dump(value)
        }
    }

    #[inline]
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        if value == unsafe { NONE_PY_TYPE } {
            ffi!(Py_INCREF(NONE_PY_TYPE));
            Ok(value)
        } else {
            self.encoder.load(value)
        }
    }
}

#[derive(Debug, Clone)]
pub struct TupleEncoder {
    pub(crate) encoders: Vec<Box<TEncoder>>,
}

impl Encoder for TupleEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        let len = py_len(value)?;
        if len != self.encoders.len() as isize {
            return Err(ValidationError::new_err(
                "Invalid number of items for tuple",
            ));
        }
        let list = ffi!(PyList_New(len));
        for i in 0..len {
            let item = ffi!(PySequence_GetItem(value, i)); // RC +1
            let val = self.encoders[i as usize].dump(item)?; // new obj or RC +1
            ffi!(PyList_SetItem(list, i, val)); // RC not changed
            ffi!(Py_DECREF(item));
        }
        Ok(list)
    }

    #[inline]
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        let len = py_len(value)?;
        if len != self.encoders.len() as isize {
            return Err(ValidationError::new_err(
                "Invalid number of items for tuple",
            ));
        }

        let list = ffi!(PyTuple_New(len));
        for i in 0..len {
            let item = ffi!(PyList_GetItem(value, i)); // RC not changed
            let val = self.encoders[i as usize].load(item)?; // new obj or RC +1
            ffi!(PyTuple_SetItem(list, i, val)); // RC not changed
        }
        Ok(list)
    }
}

#[derive(Debug, Clone)]
pub struct TimeEncoder;

impl Encoder for TimeEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        call_method!(value, ISOFORMAT_STR)
    }

    #[inline]
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        parse_time(py_str_to_str(value)?)
    }
}

#[derive(Debug, Clone)]
pub struct DateTimeEncoder;

impl Encoder for DateTimeEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        call_method!(value, ISOFORMAT_STR)
    }

    #[inline]
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        parse_datetime(py_str_to_str(value)?)
    }
}

#[derive(Debug, Clone)]
pub struct DateEncoder;

impl Encoder for DateEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        call_method!(value, ISOFORMAT_STR)
    }

    #[inline]
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        parse_date(py_str_to_str(value)?)
    }
}

#[derive(Debug, Clone)]
pub struct LazyEncoder {
    pub(crate) inner: Arc<AtomicRefCell<Option<EntityEncoder>>>,
}

impl Encoder for LazyEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        match self.inner.borrow().as_ref() {
            Some(encoder) => encoder.dump(value),
            None => Err(PyRuntimeError::new_err(
                "[RUST] Invalid recursive encoder".to_string(),
            )),
        }
    }

    #[inline]
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        match self.inner.borrow().as_ref() {
            Some(encoder) => encoder.load(value),
            None => Err(PyRuntimeError::new_err(
                "[RUST] Invalid recursive encoder".to_string(),
            )),
        }
    }
}
