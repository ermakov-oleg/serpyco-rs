use crate::serializer::py::{create_new_object, decimal, py_len};
use pyo3::exceptions::PyException;
use pyo3::types::{PyDict, PyList, PyTuple, PyUnicode};
use pyo3::{pyclass, pymethods, Py, PyAny, PyResult, ToPyObject};
use std::fmt::Debug;

pyo3::create_exception!(serpyco_rs, ValidationError, PyException);

pub trait Encoder: Debug {
    fn dump(&self, value: &PyAny) -> PyResult<Py<PyAny>>;
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
        self.encoder.dump(value)
    }
    pub fn load(&self, value: &PyAny) -> PyResult<Py<PyAny>> {
        self.encoder.load(value)
    }
}

#[derive(Debug)]
pub struct NoopEncoder;

impl Encoder for NoopEncoder {
    #[inline]
    fn dump(&self, value: &PyAny) -> PyResult<Py<PyAny>> {
        Ok(value.into())
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
    fn dump(&self, value: &PyAny) -> PyResult<Py<PyAny>> {
        Ok(value.into())
    }

    #[inline]
    fn load(&self, value: &PyAny) -> PyResult<Py<PyAny>> {
        match decimal(value.py())?.call1((value,)) {
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
    fn dump(&self, value: &PyAny) -> PyResult<Py<PyAny>> {
        let result = PyDict::new(value.py());
        for i in value.call_method0("items")?.iter()? {
            let item = i?.downcast::<PyTuple>()?;
            let key = &item[0];
            let value = &item[1];
            result.set_item(self.key_encoder.dump(key)?, self.value_encoder.dump(value)?)?
        }

        Ok(result.into())
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
    fn dump(&self, value: &PyAny) -> PyResult<Py<PyAny>> {
        let iterable = value.iter()?;
        let result = PyList::empty(value.py());
        for i in iterable {
            result.append(self.encoder.dump(i.unwrap())?)?;
        }

        Ok(result.into())
    }

    #[inline]
    fn load(&self, value: &PyAny) -> PyResult<Py<PyAny>> {
        let iterable = value.iter()?;
        let len: usize = py_len(value)?.extract()?;
        let mut result = Vec::with_capacity(len);
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
    pub(crate) name: Py<PyUnicode>,
    pub(crate) dict_key: Py<PyUnicode>,
    pub(crate) encoder: Box<dyn Encoder + Send>,
    pub(crate) default: Option<Py<PyAny>>,
    pub(crate) default_factory: Option<Py<PyAny>>,
}

impl Encoder for EntityEncoder {
    #[inline]
    fn dump(&self, value: &PyAny) -> PyResult<Py<PyAny>> {
        let kwargs = PyDict::new(value.py());

        for field in &self.fields {
            let val = value.getattr(&field.name)?;
            let dump_result = field.encoder.dump(val.into())?;
            kwargs.set_item(&field.dict_key, dump_result)?
        }
        Ok(Py::from(kwargs))
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
            obj.setattr(&field.name, val)?;
        }
        Ok(Py::from(obj))
    }
}
