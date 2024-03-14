use pyo3::prelude::PyAnyMethods;
use pyo3::types::{PyDate, PyDateTime, PyDelta, PyTime, PyTzInfo};
use pyo3::{Bound, PyErr, PyResult, Python};
use pyo3_ffi::PyTimeZone_FromOffset;
use speedate::{Date, DateTime, ParseError, Time};

use crate::errors::{ToPyErr, ValidationError};

#[inline]
pub(crate) fn parse_datetime<'py>(
    py: Python<'py>,
    value: &str,
) -> PyResult<Bound<'py, PyDateTime>> {
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
pub(crate) fn parse_time<'py>(py: Python<'py>, value: &str) -> PyResult<Bound<'py, PyTime>> {
    let time = Time::parse_str(value).map_err(InnerParseError::from)?;
    PyTime::new_bound(
        py,
        time.hour,
        time.minute,
        time.second,
        time.microsecond,
        time_as_tzinfo(py, &time)?.as_ref(),
    )
}

#[inline]
pub(crate) fn parse_date<'py>(py: Python<'py>, value: &str) -> PyResult<Bound<'py, PyDate>> {
    let date = Date::parse_str(value).map_err(InnerParseError::from)?;
    PyDate::new_bound(py, date.year.into(), date.month, date.day)
}

#[inline]
fn time_as_tzinfo<'py>(py: Python<'py>, time: &Time) -> PyResult<Option<Bound<'py, PyTzInfo>>> {
    match time.tz_offset {
        Some(offset) => {
            let delta = PyDelta::new_bound(py, 0, offset, 0, true)?;

            let tzinfo =
                unsafe { Bound::from_owned_ptr(py, PyTimeZone_FromOffset(delta.as_ptr())) };

            Ok(Some(tzinfo.downcast_into()?))
        }
        None => Ok(None),
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
