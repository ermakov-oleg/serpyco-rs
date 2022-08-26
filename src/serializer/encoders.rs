use std::ffi::CStr;
use std::fmt::{format, Debug, Formatter};

use pyo3::exceptions::PyTypeError;
use pyo3::ffi::PyTypeObject;
use pyo3::prelude::*;
use pyo3::types::{
    PyDict, PyInt, PyIterator, PyList, PyModule, PySequence, PyTuple, PyType, PyUnicode,
};
use pyo3::{Py, PyAny, PyResult};
use pyo3::{PyObject, Python};

use crate::serializer::types;

#[pyclass]
#[derive(Debug)]
pub struct Serializer {
    py_class: Py<PyAny>,
    fields: Vec<Field>,
}

#[pymethods]
impl Serializer {
    fn __repr__(&self) -> String {
        format!("<Serializer: {:?}>", self)
    }

    fn dump(&self, value: &PyAny) -> PyResult<Py<PyAny>> {
        let mut kwargs = PyDict::new(value.py());

        for field in &self.fields {
            let val = value.getattr(&field.name)?;
            let dump_result = field.encoder.dump(val.into())?;
            kwargs.set_item(&field.dict_key, dump_result)?
        }
        Ok(Py::from(kwargs))
    }
}

#[derive(Debug)]
struct Field {
    name: Py<PyUnicode>,
    dict_key: Py<PyUnicode>,
    encoder: Box<dyn Encoder + Send>, // todo: opt out dyn dispatch
    default: Option<Py<PyAny>>,
    default_factory: Option<Py<PyAny>>,
}

#[pyfunction]
pub fn make_serializer(py_class: &PyAny) -> PyResult<Serializer> {
    let typing = PyModule::import(py_class.py(), "typing")?;
    let type_hints: &PyDict = typing
        .getattr("get_type_hints")?
        .call1((py_class,))?
        .downcast()?;

    let dataclasses = PyModule::import(py_class.py(), "dataclasses")?;
    let class_fields: &PyTuple = dataclasses
        .getattr("fields")?
        .call1((py_class,))?
        .downcast()?;

    let mut fields = vec![];

    for field in class_fields.iter() {
        let name: &PyUnicode = field.getattr("name")?.downcast()?;
        let field_type = type_hints.get_item(name).unwrap();

        let (default, default_factory) = get_defaults(field, field_type)?;

        let fld = Field {
            name: name.into(),
            dict_key: name.into(),
            encoder: get_encoder_for_type(field_type)?,
            default: default.map(|d| d.into()),
            default_factory: default_factory.map(|d| d.into()),
        };
        fields.push(fld);
    }

    let serializer = Serializer {
        py_class: py_class.into(),
        fields,
    };

    Ok(serializer)
}

#[derive(Clone, Eq, PartialEq)]
pub enum ObjectType {
    Str,
    Int,
    Bool,
    None,
    Float,
    DataClass,
    Iterable,
    // Dict,
    // Tuple,
    // Enum,
    Unknown(String),
}

pub trait Encoder: Debug {
    fn dump(&self, value: &PyAny) -> PyResult<Py<PyAny>>;
    // fn loads(value: Py<PyAny>) -> &PyAny where Self: Sized;
}

#[derive(Debug)]
struct BuiltinsEncoder;

impl Encoder for BuiltinsEncoder {
    fn dump(&self, value: &PyAny) -> PyResult<Py<PyAny>> {
        Ok(value.into())
    }
}

#[derive(Debug)]
struct DataClassEncoder {
    serializer: Serializer,
}

impl Encoder for DataClassEncoder {
    fn dump(&self, value: &PyAny) -> PyResult<Py<PyAny>> {
        self.serializer.dump(value).into()
    }
}

#[derive(Debug)]
struct IterableFieldEncoder {
    encoder: Box<dyn Encoder + Send>,
}

impl Encoder for IterableFieldEncoder {
    fn dump(&self, value: &PyAny) -> PyResult<Py<PyAny>> {
        let iterable = value.iter()?;
        let mut result = PyList::empty(value.py());
        for i in iterable {
            result.append(self.encoder.dump(i.unwrap())?)?;
        }

        Ok(result.into())
    }
}

pub fn get_encoder_for_type(type_: &PyAny) -> PyResult<Box<dyn Encoder + Send>> {
    let mut type_ = type_;
    if is_union_type(type_)? {
        let args = get_args(type_)?;
        type_ = TMP_get_arg1(args)?;
    }
    let obj_type = get_object_type(type_)?;

    let encoder: Box<dyn Encoder + Send> = match obj_type {
        ObjectType::None
        | ObjectType::Bool
        | ObjectType::Str
        | ObjectType::Int
        | ObjectType::Float => Box::new(BuiltinsEncoder),
        ObjectType::DataClass => Box::new(DataClassEncoder {
            serializer: make_serializer(type_)?,
        }),
        ObjectType::Iterable => {
            let items_type = TMP_get_arg1(get_args(type_)?)?;

            Box::new(IterableFieldEncoder {
                encoder: get_encoder_for_type(items_type)?,
            })
        }
        ObjectType::Unknown(t) => {
            todo!("Unknown type: {}", t)
        }
    };

    Ok(encoder)
}

fn get_object_type(type_: &PyAny) -> PyResult<ObjectType> {
    if is_native_type(type_, unsafe { types::STR_TYPE }) {
        Ok(ObjectType::Str)
    } else if is_native_type(type_, unsafe { types::FLOAT_TYPE }) {
        Ok(ObjectType::Float)
    } else if is_native_type(type_, unsafe { types::BOOL_TYPE }) {
        Ok(ObjectType::Bool)
    } else if is_native_type(type_, unsafe { types::INT_TYPE }) {
        Ok(ObjectType::Int)
    } else if is_native_type(type_, unsafe { types::NONE_TYPE }) {
        Ok(ObjectType::None)
    } else if is_generic(type_, get_iterable_type(type_.py())?)? {
        Ok(ObjectType::Iterable)
    } else if is_dataclass(type_)? {
        Ok(ObjectType::DataClass)
        // } else if object_type == unsafe { types::LIST_TYPE } {
        //     Ok(ObjectType::List)
        // } else if object_type == unsafe { types::TUPLE_TYPE } {
        //     Ok(ObjectType::Tuple)
        // } else if object_type == unsafe { types::DICT_TYPE } {
        //     Ok(ObjectType::Dict)
        // } else if is_enum_subclass(object_type) {
        //     Ok(ObjectType::Enum)
    } else {
        Ok(ObjectType::Unknown(type_.to_string()))
    }
}

fn is_native_type(type_: &PyAny, native_type: *mut PyTypeObject) -> bool {
    match type_.downcast::<PyType>() {
        Ok(object_type) => object_type.as_type_ptr() == unsafe { native_type },
        Err(_) => false,
    }
}

fn is_dataclass(py_class: &PyAny) -> PyResult<bool> {
    let dataclasses = PyModule::import(py_class.py(), "dataclasses")?;
    let result = dataclasses.getattr("is_dataclass")?.call1((py_class,))?;
    result.is_true()
}

fn is_dataclass_missing(value: &PyAny) -> PyResult<bool> {
    let dataclasses = PyModule::import(value.py(), "dataclasses")?;
    let missing = dataclasses.getattr("MISSING")?;
    Ok(value.is(missing))
}

fn get_defaults<'a>(
    field: &'a PyAny,
    field_type: &'_ PyAny,
) -> PyResult<(Option<&'a PyAny>, Option<&'a PyAny>)> {
    let default = field.getattr("default")?;
    let default_factory = field.getattr("default_factory")?;
    if is_dataclass_missing(default)?
        && is_dataclass_missing(default_factory)?
        && is_optional(field_type)?
    {
        return Ok((None, Some(default_factory)));
    }

    Ok((Some(default), Some(default_factory)))
}

fn is_optional(field_type: &PyAny) -> PyResult<bool> {
    let typing_inspect = PyModule::import(field_type.py(), "typing_inspect")?;
    typing_inspect
        .getattr("is_optional_type")?
        .call1((field_type,))?
        .is_true()
}

fn is_union_type(field_type: &PyAny) -> PyResult<bool> {
    let typing_inspect = PyModule::import(field_type.py(), "typing_inspect")?;
    typing_inspect
        .getattr("is_union_type")?
        .call1((field_type,))?
        .is_true()
}

fn get_args(field_type: &PyAny) -> PyResult<&PyTuple> {
    let typing_inspect = PyModule::import(field_type.py(), "typing_inspect")?;
    Ok(typing_inspect
        .getattr("get_args")?
        .call1((field_type,))?
        .downcast()?)
}

fn is_none_type(type_: &PyAny) -> bool {
    match get_object_type(type_) {
        Ok(t) => t == ObjectType::None,
        Err(_) => false,
    }
}

fn is_generic(field_type: &PyAny, types: &PyAny) -> PyResult<bool> {
    let typing_inspect = PyModule::import(field_type.py(), "typing_inspect")?;

    let is_generic_type = typing_inspect
        .getattr("is_generic_type")?
        .call1((field_type,))?
        .is_true()?;
    let is_tuple_type = typing_inspect
        .getattr("is_tuple_type")?
        .call1((field_type,))?
        .is_true()?;

    if is_generic_type || is_tuple_type {
        let origin = typing_inspect.getattr("get_origin")?.call1((field_type,))?;
        is_subclass(origin, types)
    } else {
        Ok(false)
    }
}

fn get_iterable_type(py: Python) -> PyResult<&PyAny> {
    let typing = PyModule::import(py, "typing")?;
    typing.getattr("Iterable")
}

fn is_subclass(cls: &PyAny, types: &PyAny) -> PyResult<bool> {
    let builtins = PyModule::import(cls.py(), "builtins")?;
    builtins.getattr("issubclass")?.call1((cls, types))?.is_true()
}

fn TMP_get_arg1(args: &PyTuple) -> PyResult<&PyAny> {
    let filtered: Vec<&PyAny> = args.iter().filter(|it| !is_none_type(it)).collect();
    // if filtered.len() > 1 {
    //     Err(PyTypeError::new_err("Invalid args"))
    // } else {
    Ok(filtered[0])
    // }
}
