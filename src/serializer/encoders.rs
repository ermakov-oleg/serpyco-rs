use crate::python::{
    call_isoformat, create_new_object, datetime_to_date, get_none, get_value_attr, is_datetime,
    is_none, iter_over_dict_items, obj_to_str, parse_date, parse_datetime, parse_number,
    parse_time, py_len, py_object_call1_make_tuple_or_err, py_object_get_attr, py_object_get_item,
    py_object_set_attr, py_str_to_str, py_tuple_get_item, to_bool, to_decimal, to_uuid,
    unicode_from_str,
};
use atomic_refcell::AtomicRefCell;
use pyo3::exceptions::PyRuntimeError;
use pyo3::types::PyString;
use pyo3::{AsPyPointer, Py, PyAny, PyResult};
use pyo3_ffi::PyObject;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

use crate::python::macros::{call_object, ffi};

use crate::errors::{ToPyErr, ValidationError};
use dyn_clone::{clone_trait_object, DynClone};

pub type TEncoder = dyn Encoder + Send + Sync;

pub trait Encoder: DynClone + Debug {
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject>;
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject>;
    fn load_value(&self, value: Value) -> PyResult<*mut PyObject>;
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

    fn load_value(&self, value: Value) -> PyResult<*mut PyObject> {
        match value {
            Value::Null => Ok(get_none()),
            Value::Bool(bool) => Ok(to_bool(bool)),
            Value::Number(number) => Ok(parse_number(number)),
            Value::String(string) => Ok(unicode_from_str(&string)),
            Value::Array(_) | Value::Object(_) => {
                Err(ValidationError::new_err("invalid value type"))
            }
        }
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

    #[inline]
    fn load_value(&self, value: Value) -> PyResult<*mut PyObject> {
        if let Value::String(string) = value {
            let py_string = unicode_from_str(&string);
            self.load(py_string)
        } else {
            Err(ValidationError::new_err("invalid value type"))
        }
    }
}

#[derive(Debug, Clone)]
pub struct DictionaryEncoder {
    pub key_encoder: Box<TEncoder>,
    pub value_encoder: Box<TEncoder>,
    pub omit_none: bool,
}

impl Encoder for DictionaryEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        let dict_ptr = ffi!(PyDict_New());

        for i in iter_over_dict_items(value)? {
            // items RC +1
            let item = i?;
            let key = self.key_encoder.dump(py_tuple_get_item(item, 0)?)?; // new obj or RC +1
            let value = self.value_encoder.dump(py_tuple_get_item(item, 1)?)?; // new obj or RC +1

            if !self.omit_none || !is_none(value) {
                ffi!(PyDict_SetItem(dict_ptr, key, value)); // key and val or RC +1
            }
            ffi!(Py_DECREF(key));
            ffi!(Py_DECREF(value));
            ffi!(Py_DECREF(item));
        }
        Ok(dict_ptr)
    }

    #[inline]
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        let dict_ptr = ffi!(PyDict_New());

        for i in iter_over_dict_items(value)? {
            // items RC +1
            let item = i?;
            let key = self.key_encoder.load(py_tuple_get_item(item, 0)?)?; // new obj or RC +1
            let value = self.value_encoder.load(py_tuple_get_item(item, 1)?)?; // new obj or RC +1
            ffi!(PyDict_SetItem(dict_ptr, key, value)); // key and val or RC +1
            ffi!(Py_DECREF(key));
            ffi!(Py_DECREF(value));
            ffi!(Py_DECREF(item));
        }

        Ok(dict_ptr)
    }
    #[inline]
    fn load_value(&self, value: Value) -> PyResult<*mut PyObject> {
        if let Value::Object(object) = value {
            let dict_ptr = ffi!(PyDict_New());
            for (key, value) in object {
                let key = self.key_encoder.load_value(Value::String(key))?;
                let value = self.value_encoder.load_value(value)?;
                ffi!(PyDict_SetItem(dict_ptr, key, value));
                ffi!(Py_DECREF(key));
                ffi!(Py_DECREF(value));
            }
            Ok(dict_ptr)
        } else {
            Err(ValidationError::new_err("invalid value type"))
        }
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

    #[inline]
    fn load_value(&self, value: Value) -> PyResult<*mut PyObject> {
        if let Value::Array(array) = value {
            let list = ffi!(PyList_New(array.len().try_into().unwrap()));
            for (i, item) in array.into_iter().enumerate() {
                let val = self.encoder.load_value(item)?;
                ffi!(PyList_SetItem(list, i as isize, val));
            }
            Ok(list)
        } else {
            Err(ValidationError::new_err("invalid value type"))
        }
    }
}

#[derive(Debug, Clone)]
pub struct EntityEncoder {
    pub(crate) cls: Py<PyAny>,
    pub(crate) omit_none: bool,
    pub(crate) fields: Vec<Field>,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub(crate) name: Py<PyString>,
    pub(crate) dict_key: Py<PyString>,
    pub(crate) dict_key_rs: String,
    pub(crate) encoder: Box<TEncoder>,
    pub(crate) required: bool,
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

            if field.required || !self.omit_none || !is_none(dump_result) {
                ffi!(PyDict_SetItem(
                    dict_ptr,
                    field.dict_key.as_ptr(),
                    dump_result
                )); // key and val RC +1
            }

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

    #[inline]
    fn load_value(&self, value: Value) -> PyResult<*mut PyObject> {
        if let Value::Object(mut object) = value {
            let obj = create_new_object(self.cls.as_ptr())?;
            for field in &self.fields {
                let val = match object.remove(&field.dict_key_rs) {
                    Some(val) => field.encoder.load_value(val)?, // new obj or RC +1
                    None => match (&field.default, &field.default_factory) {
                        (Some(val), _) => val.clone().as_ptr(),
                        (_, Some(val)) => call_object!(val.as_ptr())?,
                        (None, _) => {
                            return Err(ValidationError::new_err(format!(
                                "data dictionary is missing required parameter {}",
                                &field.name
                            )))
                        }
                    },
                };
                py_object_set_attr(obj, field.name.as_ptr(), val)?; // val RC +1
                ffi!(Py_DECREF(val));
            }
            Ok(obj)
        } else {
            Err(ValidationError::new_err("invalid value type"))
        }
    }
}

#[derive(Debug, Clone)]
pub struct TypedDictEncoder {
    pub(crate) omit_none: bool,
    pub(crate) fields: Vec<Field>,
}

impl Encoder for TypedDictEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        let dict_ptr = ffi!(PyDict_New());

        for field in &self.fields {
            let field_val = match py_object_get_item(value, field.name.as_ptr()) {
                Ok(val) => val,
                Err(e) => {
                    if field.required {
                        return Err(ValidationError::new_err(format!(
                            "data dictionary is missing required parameter {} (err: {})",
                            &field.name, e
                        )));
                    } else {
                        continue;
                    }
                }
            }; // val RC +1

            let dump_result = field.encoder.dump(field_val)?; // new obj or RC +1

            if field.required || !self.omit_none || !is_none(dump_result) {
                ffi!(PyDict_SetItem(
                    dict_ptr,
                    field.dict_key.as_ptr(),
                    dump_result
                )); // key and val RC +1
            }

            ffi!(Py_DECREF(field_val));
            ffi!(Py_DECREF(dump_result));
        }

        Ok(dict_ptr)
    }

    #[inline]
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        let dict_ptr = ffi!(PyDict_New());
        for field in &self.fields {
            let val = match py_object_get_item(value, field.dict_key.as_ptr()) {
                Ok(val) => field.encoder.load(val)?, // new obj or RC +1
                Err(e) => {
                    if field.required {
                        return Err(ValidationError::new_err(format!(
                            "data dictionary is missing required parameter {} (err: {})",
                            &field.dict_key, e
                        )));
                    } else {
                        continue;
                    }
                }
            };
            ffi!(PyDict_SetItem(dict_ptr, field.name.as_ptr(), val)); // key and val RC +1
            ffi!(Py_DECREF(val));
        }
        Ok(dict_ptr)
    }

    #[inline]
    fn load_value(&self, value: Value) -> PyResult<*mut PyObject> {
        if let Value::Object(mut object) = value {
            let dict_ptr = ffi!(PyDict_New());
            for field in &self.fields {
                let val = match object.remove(&field.dict_key_rs) {
                    Some(val) => field.encoder.load_value(val)?, // new obj or RC +1
                    None => {
                        if field.required {
                            return Err(ValidationError::new_err(format!(
                                "data dictionary is missing required parameter {}",
                                &field.dict_key
                            )));
                        } else {
                            continue;
                        }
                    }
                };
                ffi!(PyDict_SetItem(dict_ptr, field.name.as_ptr(), val)); // key and val RC +1
                ffi!(Py_DECREF(val));
            }
            Ok(dict_ptr)
        } else {
            Err(ValidationError::new_err("invalid value type"))
        }
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
        to_uuid(value)
    }

    #[inline]
    fn load_value(&self, value: Value) -> PyResult<*mut PyObject> {
        if let Value::String(s) = value {
            let py_string = unicode_from_str(&s);
            self.load(py_string)
        } else {
            Err(ValidationError::new_err("invalid value type"))
        }
    }
}

#[derive(Debug, Clone)]
pub struct EnumEncoder {
    pub(crate) enum_type: pyo3::PyObject,
}

impl Encoder for EnumEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        get_value_attr(value)
    }

    #[inline]
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        py_object_call1_make_tuple_or_err(self.enum_type.as_ptr(), value)
    }

    #[inline]
    fn load_value(&self, value: Value) -> PyResult<*mut PyObject> {
        if let Value::String(s) = value {
            let py_string = unicode_from_str(&s);
            self.load(py_string)
        } else if let Value::Number(n) = value {
            let py_number = parse_number(n);
            self.load(py_number)
        } else {
            Err(ValidationError::new_err("invalid value type"))
        }
    }
}

#[derive(Debug, Clone)]
pub struct OptionalEncoder {
    pub(crate) encoder: Box<TEncoder>,
}

impl Encoder for OptionalEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        if is_none(value) {
            Ok(get_none())
        } else {
            self.encoder.dump(value)
        }
    }

    #[inline]
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        if is_none(value) {
            Ok(get_none())
        } else {
            self.encoder.load(value)
        }
    }

    #[inline]
    fn load_value(&self, value: Value) -> PyResult<*mut PyObject> {
        if value == Value::Null {
            Ok(get_none())
        } else {
            self.encoder.load_value(value)
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

        let tuple = ffi!(PyTuple_New(len));
        for i in 0..len {
            let item = ffi!(PySequence_GetItem(value, i)); // RC +1
            let val = self.encoders[i as usize].load(item)?; // new obj or RC +1
            ffi!(PyTuple_SetItem(tuple, i, val)); // RC not changed
            ffi!(Py_DECREF(item));
        }
        Ok(tuple)
    }

    #[inline]
    fn load_value(&self, value: Value) -> PyResult<*mut PyObject> {
        if let Value::Array(items) = value {
            let len = items.len();
            if len != self.encoders.len() {
                return Err(ValidationError::new_err(
                    "Invalid number of items for tuple",
                ));
            }

            let tuple = ffi!(PyTuple_New(len as isize));
            for (i, val) in items.into_iter().enumerate() {
                let item = self.encoders[i].load_value(val)?;
                ffi!(PyTuple_SetItem(tuple, i as isize, item));
            }
            Ok(tuple)
        } else {
            Err(ValidationError::new_err("invalid value type"))
        }
    }
}

#[derive(Debug, Clone)]
pub struct UnionEncoder {
    pub(crate) encoders: HashMap<String, Box<TEncoder>>,
    pub(crate) dump_discriminator: Py<PyString>,
    pub(crate) load_discriminator: Py<PyString>,
    pub(crate) load_discriminator_rs: String,
}

impl Encoder for UnionEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        let discriminator = py_object_get_attr(value, self.dump_discriminator.as_ptr())?; // val RC +1
        let key = py_str_to_str(discriminator)?;
        let encoder = self
            .encoders
            .get(key)
            .ok_or(ValidationError::new_err(format!(
                "No encoder for '{key}' discriminator"
            )))?;
        ffi!(Py_DECREF(discriminator));
        encoder.dump(value)
    }

    #[inline]
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        let discriminator = py_object_get_item(value, self.load_discriminator.as_ptr())?; // val RC +1
        let key = py_str_to_str(discriminator)?;
        let encoder = self
            .encoders
            .get(key)
            .ok_or(ValidationError::new_err(format!(
                "No encoder for '{key}' discriminator"
            )))?;
        ffi!(Py_DECREF(discriminator));
        encoder.load(value)
    }

    #[inline]
    fn load_value(&self, value: Value) -> PyResult<*mut PyObject> {
        if let Value::Object(obj) = value {
            let discriminator = obj
                .get(&self.load_discriminator_rs)
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .ok_or(ValidationError::new_err("missing discriminator"))?;
            let encoder = self
                .encoders
                .get(&discriminator)
                .ok_or(ValidationError::new_err(format!(
                    "No encoder for '{discriminator}' discriminator"
                )))?;
            encoder.load_value(Value::Object(obj))
        } else {
            Err(ValidationError::new_err("invalid value type"))
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimeEncoder;

impl Encoder for TimeEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        call_isoformat(value)
    }

    #[inline]
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        parse_time(py_str_to_str(value)?)
    }

    #[inline]
    fn load_value(&self, value: Value) -> PyResult<*mut PyObject> {
        if let Value::String(s) = value {
            parse_time(&s)
        } else {
            Err(ValidationError::new_err("invalid value type"))
        }
    }
}

#[derive(Debug, Clone)]
pub struct DateTimeEncoder;

impl Encoder for DateTimeEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        call_isoformat(value)
    }

    #[inline]
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        parse_datetime(py_str_to_str(value)?)
    }

    #[inline]
    fn load_value(&self, value: Value) -> PyResult<*mut PyObject> {
        if let Value::String(s) = value {
            parse_datetime(&s)
        } else {
            Err(ValidationError::new_err("invalid value type"))
        }
    }
}

#[derive(Debug, Clone)]
pub struct DateEncoder;

impl Encoder for DateEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        let date = if is_datetime(value) {
            datetime_to_date(value)?
        } else {
            value
        };
        call_isoformat(date)
    }

    #[inline]
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        parse_date(py_str_to_str(value)?)
    }

    #[inline]
    fn load_value(&self, value: Value) -> PyResult<*mut PyObject> {
        if let Value::String(s) = value {
            parse_date(&s)
        } else {
            Err(ValidationError::new_err("invalid value type"))
        }
    }
}

#[derive(Debug)]
pub enum Encoders {
    Entity(EntityEncoder),
    TypedDict(TypedDictEncoder),
}

#[derive(Debug, Clone)]
pub struct LazyEncoder {
    pub(crate) inner: Arc<AtomicRefCell<Option<Encoders>>>,
}

impl Encoder for LazyEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        match self.inner.borrow().as_ref() {
            Some(encoder) => match encoder {
                Encoders::Entity(encoder) => encoder.dump(value),
                Encoders::TypedDict(encoder) => encoder.dump(value),
            },
            None => Err(PyRuntimeError::new_err(
                "[RUST] Invalid recursive encoder".to_string(),
            )),
        }
    }

    #[inline]
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        match self.inner.borrow().as_ref() {
            Some(encoder) => match encoder {
                Encoders::Entity(encoder) => encoder.load(value),
                Encoders::TypedDict(encoder) => encoder.load(value),
            },
            None => Err(PyRuntimeError::new_err(
                "[RUST] Invalid recursive encoder".to_string(),
            )),
        }
    }

    #[inline]
    fn load_value(&self, value: Value) -> PyResult<*mut PyObject> {
        match self.inner.borrow().as_ref() {
            Some(encoder) => match encoder {
                Encoders::Entity(encoder) => encoder.load_value(value),
                Encoders::TypedDict(encoder) => encoder.load_value(value),
            },
            None => Err(PyRuntimeError::new_err(
                "[RUST] Invalid recursive encoder".to_string(),
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CustomEncoder {
    pub(crate) inner: Box<TEncoder>,
    pub(crate) dump: Option<Py<PyAny>>,
    pub(crate) load: Option<Py<PyAny>>,
}

impl Encoder for CustomEncoder {
    #[inline]
    fn dump(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        match self.dump {
            Some(ref dump) => py_object_call1_make_tuple_or_err(dump.as_ptr(), value),
            None => self.inner.dump(value),
        }
    }

    #[inline]
    fn load(&self, value: *mut PyObject) -> PyResult<*mut PyObject> {
        match self.load {
            Some(ref load) => py_object_call1_make_tuple_or_err(load.as_ptr(), value),
            None => self.inner.load(value),
        }
    }

    #[inline]
    fn load_value(&self, value: Value) -> PyResult<*mut PyObject> {
        self.inner.load_value(value)
    }
}
