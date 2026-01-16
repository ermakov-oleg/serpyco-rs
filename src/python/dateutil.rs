use pyo3::prelude::PyAnyMethods;
use pyo3::types::{
    PyDate, PyDateAccess, PyDateTime, PyDelta, PyDeltaAccess, PyTime, PyTimeAccess, PyTzInfo,
    PyTzInfoAccess,
};
use pyo3::{Bound, PyErr, PyResult, Python};
use pyo3_ffi::PyTimeZone_FromOffset;
use speedate::{
    Date, DateTime, MicrosecondsPrecisionOverflowBehavior, ParseError, Time, TimeConfig,
};

use crate::errors::{ToPyErr, ValidationError};

const TIME_CONFIG: TimeConfig = TimeConfig {
    microseconds_precision_overflow_behavior: MicrosecondsPrecisionOverflowBehavior::Truncate,
    unix_timestamp_offset: None,
};

#[inline]
pub(crate) fn parse_datetime<'py>(
    py: Python<'py>,
    value: &str,
) -> PyResult<Bound<'py, PyDateTime>> {
    let datetime = DateTime::parse_bytes_rfc3339_with_config(value.as_ref(), &TIME_CONFIG)
        .map_err(InnerParseError::from)?;
    PyDateTime::new(
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
    let time = Time::parse_bytes_with_config(value.as_ref(), &TIME_CONFIG)
        .map_err(InnerParseError::from)?;
    PyTime::new(
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
    PyDate::new(py, date.year.into(), date.month, date.day)
}

#[inline]
fn time_as_tzinfo<'py>(py: Python<'py>, time: &Time) -> PyResult<Option<Bound<'py, PyTzInfo>>> {
    match time.tz_offset {
        Some(offset) => {
            let delta = PyDelta::new(py, 0, offset, 0, true)?;

            let tzinfo =
                unsafe { Bound::from_owned_ptr(py, PyTimeZone_FromOffset(delta.as_ptr())) };

            Ok(Some(tzinfo.cast_into()?))
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

pub(crate) fn dump_datetime(
    value: &Bound<PyDateTime>,
    naive_datetime_to_utc: bool,
) -> PyResult<String> {
    let date = to_date(value);
    let mut time = to_time(value);
    let tz_offset = to_tz_offset(value, Some(value))?;
    match tz_offset {
        Some(offset) => {
            time.tz_offset = Some(offset);
        }
        None if naive_datetime_to_utc => {
            time.tz_offset = Some(0);
        }
        None => {}
    }
    Ok(DateTime { date, time }.to_string())
}

pub(crate) fn dump_time(value: &Bound<PyTime>) -> PyResult<String> {
    let mut time = to_time(value);
    let tz_offset = to_tz_offset(value, None)?;
    if let Some(offset) = tz_offset {
        time.tz_offset = Some(offset);
    }
    Ok(time.to_string())
}

pub(crate) fn dump_date(value: &Bound<PyDate>) -> String {
    let date = to_date(value);
    date.to_string()
}

fn to_date(value: &dyn PyDateAccess) -> Date {
    Date {
        year: value.get_year() as u16,
        month: value.get_month(),
        day: value.get_day(),
    }
}

fn to_time(value: &dyn PyTimeAccess) -> Time {
    Time {
        hour: value.get_hour(),
        minute: value.get_minute(),
        second: value.get_second(),
        microsecond: value.get_microsecond(),
        tz_offset: None,
    }
}

fn to_tz_offset(
    value: &dyn PyTzInfoAccess,
    datetime: Option<&Bound<PyDateTime>>,
) -> PyResult<Option<i32>> {
    let tzinfo = value.get_tzinfo();
    if tzinfo.is_none() {
        return Ok(None);
    }
    let offset = tzinfo.unwrap().call_method1("utcoffset", (datetime,))?;
    if offset.is_none() {
        return Ok(None);
    }
    let offset = offset.cast::<PyDelta>()?;
    Ok(Some(offset.get_days() * 86400 + offset.get_seconds()))
}
