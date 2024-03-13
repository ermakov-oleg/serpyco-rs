use std::os::raw::c_int;

use pyo3::{Bound, PyErr, PyResult, Python};
use pyo3::prelude::PyAnyMethods;
use pyo3::types::{PyDate, PyDateTime, PyDelta, PyTime, PyTzInfo};
use pyo3_ffi::{PyObject, PyTimeZone_FromOffset};
use speedate::{Date, DateTime, ParseError, Time};

use super::py::from_ptr_or_err;
use super::types::NONE_PY_TYPE;

use crate::errors::{ToPyErr, ValidationError};

#[inline]
pub(crate) fn parse_time(value: &str) -> PyResult<*mut PyObject> {
    let time = Time::parse_str(value).map_err(InnerParseError::from)?;
    let api = ensure_datetime_api();
    unsafe {
        let ptr = (api.Time_FromTime)(
            c_int::from(time.hour),
            c_int::from(time.minute),
            c_int::from(time.second),
            time.microsecond as i32,
            py_timezone(time.tz_offset)?,
            api.TimeType,
        );
        from_ptr_or_err(ptr)
    }
}

#[inline]
pub(crate) fn parse_date(value: &str) -> PyResult<*mut PyObject> {
    let date = Date::parse_str(value).map_err(InnerParseError::from)?;
    let api = ensure_datetime_api();
    unsafe {
        let ptr = (api.Date_FromDate)(
            c_int::from(date.year),
            c_int::from(date.month),
            c_int::from(date.day),
            api.DateType,
        );
        from_ptr_or_err(ptr)
    }
}

#[inline]
pub(crate) fn parse_datetime(value: &str) -> PyResult<*mut PyObject> {
    let datetime = DateTime::parse_str(value).map_err(InnerParseError::from)?;
    let api = ensure_datetime_api();
    let ptr = unsafe {
        (api.DateTime_FromDateAndTime)(
            c_int::from(datetime.date.year),
            c_int::from(datetime.date.month),
            c_int::from(datetime.date.day),
            c_int::from(datetime.time.hour),
            c_int::from(datetime.time.minute),
            c_int::from(datetime.time.second),
            datetime.time.microsecond as i32,
            py_timezone(datetime.time.tz_offset)?,
            api.DateTimeType,
        )
    };
    from_ptr_or_err(ptr)
}

#[inline]
pub(crate) fn parse_datetime_new<'a, 'py>(py: Python<'py>, value: &'a str) -> PyResult<Bound<'py, PyDateTime>> {
    let datetime = DateTime::parse_str(value).map_err(InnerParseError::from)?;
    PyDateTime::new_bound(
        py,
        datetime.date.year.into(),
        datetime.date.month,
        datetime.date.day,
        datetime.time.hour,
        datetime.time.minute,
        datetime.time.second,
        datetime.time.microsecond,
        time_as_tzinfo(py, &datetime.time)?.as_ref(),
    )
}

#[inline]
pub(crate) fn parse_time_new<'a, 'py>(py: Python<'py>, value: &'a str) -> PyResult<Bound<'py, PyTime>> {
    let time = Time::parse_str(value).map_err(InnerParseError::from)?;
    PyTime::new_bound(
        py,
        time.hour,
        time.minute,
        time.second,
        time.microsecond,
        time_as_tzinfo(py, &time)?.as_ref()
    )
}

#[inline]
pub(crate) fn parse_date_new<'a, 'py>(py: Python<'py>, value: &'a str) -> PyResult<Bound<'py, PyDate>> {
    let date = Date::parse_str(value).map_err(InnerParseError::from)?;
    PyDate::new_bound(
        py,
        date.year.into(),
        date.month,
        date.day,
    )
}


#[inline]
fn time_as_tzinfo<'py>(py: Python<'py>, time: &Time) -> PyResult<Option<Bound<'py, PyTzInfo>>> {
    match time.tz_offset {
        Some(offset) => {

            let delta = PyDelta::new_bound(
                py,
                0,
                offset,
                0,
                true,
            )?;

            let tzinfo = unsafe { Bound::from_owned_ptr(py, PyTimeZone_FromOffset(delta.as_ptr())) };

            Ok(Some(tzinfo.downcast_into()?))
        }
        None => Ok(None),
    }
}

#[inline]
fn ensure_datetime_api() -> &'static pyo3_ffi::PyDateTime_CAPI {
    unsafe { &*pyo3_ffi::PyDateTimeAPI() }
}

#[inline]
fn py_timezone(offset: Option<i32>) -> PyResult<*mut PyObject> {
    match offset {
        Some(offset) => {
            let api = ensure_datetime_api();
            unsafe {
                let prt = (api.Delta_FromDelta)(0, offset, 0, true as c_int, api.DeltaType);
                let delta = from_ptr_or_err(prt)?;
                Ok(PyTimeZone_FromOffset(delta))
            }
        }
        None => Ok(unsafe { NONE_PY_TYPE }),
    }
}

struct InnerParseError(ParseError);

impl From<ParseError> for InnerParseError {
    fn from(other: ParseError) -> Self {
        Self(other)
    }
}

impl From<InnerParseError> for PyErr {
    fn from(e: InnerParseError) -> Self {
        ValidationError::new_err(format!("Fail parse datetime {:?}", e.0.to_string()))
    }
}
