use std::os::raw::c_int;

use chrono::{
    DateTime, Datelike, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, Offset, ParseError,
    Timelike, Utc,
};
use pyo3::{PyErr, PyResult};
use pyo3_ffi::{PyObject, PyTimeZone_FromOffset};

use crate::serializer::types::NONE_PY_TYPE;

use super::encoders::ValidationError;
use super::py::from_ptr_or_err;

pub fn parse_time(value: &str) -> PyResult<*mut PyObject> {
    #[allow(clippy::redundant_closure)]
    let (time, tz) = NaiveTime::parse_from_str(value, "%H:%M:%S%.f")
        .or_else(|_| NaiveTime::parse_from_str(value, "%H:%M"))
        .map(|v| (v, None))
        .or_else(|_| {
            let mut datetime_raw = Utc::now().date().naive_utc().to_string();
            datetime_raw.push('T');
            datetime_raw.push_str(value);
            let datetime = DateTime::parse_from_rfc3339(&datetime_raw)?;
            let tz = datetime.offset().fix();
            Ok((datetime.time(), Some(tz)))
        })
        .map_err(|e: ParseError| InnerParseError::from(e))?;
    let api = ensure_datetime_api();
    let (micros, fold) = chrono_to_micros_and_fold(&time);
    unsafe {
        let ptr = (api.Time_FromTimeAndFold)(
            c_int::from(time.hour() as u8),
            c_int::from(time.minute() as u8),
            c_int::from(time.second() as u8),
            micros as c_int,
            tz.map(py_timezone_from_fixed_offset)
                .unwrap_or(Ok(NONE_PY_TYPE))?,
            fold as c_int,
            api.TimeType,
        );
        from_ptr_or_err(ptr)
    }
}

pub fn parse_date(value: &str) -> PyResult<*mut PyObject> {
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(InnerParseError::from)?;
    let api = ensure_datetime_api();
    unsafe {
        let ptr = (api.Date_FromDate)(
            date.year(),
            c_int::from(date.month() as u8),
            c_int::from(date.day() as u8),
            api.DateType,
        );
        from_ptr_or_err(ptr)
    }
}

pub fn parse_datetime(value: &str) -> PyResult<*mut PyObject> {
    match DateTime::parse_from_rfc3339(value) {
        Ok(datetime) => {
            let tz = datetime.offset().fix();
            let py_tz = py_timezone_from_fixed_offset(tz)?;
            make_py_datetime(datetime, datetime, Some(py_tz))
        }
        Err(_) => {
            let datetime = NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f")
                .map_err(InnerParseError::from)?;
            make_py_datetime(datetime, datetime, None)
        }
    }
}

fn make_py_datetime(
    date: impl Datelike,
    time: impl Timelike,
    tz: Option<*mut PyObject>,
) -> PyResult<*mut PyObject> {
    let (micros, fold) = chrono_to_micros_and_fold(&time);
    let api = ensure_datetime_api();
    let ptr = unsafe {
        (api.DateTime_FromDateAndTimeAndFold)(
            date.year(),
            c_int::from(date.month() as u8),
            c_int::from(date.day() as u8),
            c_int::from(time.hour() as u8),
            c_int::from(time.minute() as u8),
            c_int::from(time.second() as u8),
            micros as c_int,
            tz.unwrap_or(NONE_PY_TYPE),
            c_int::from(fold),
            api.DateTimeType,
        )
    };
    from_ptr_or_err(ptr)
}

fn chrono_to_micros_and_fold(time: &impl Timelike) -> (u32, bool) {
    if let Some(folded_nanos) = time.nanosecond().checked_sub(1_000_000_000) {
        (folded_nanos / 1000, true)
    } else {
        (time.nanosecond() / 1000, false)
    }
}

fn ensure_datetime_api() -> &'static pyo3_ffi::PyDateTime_CAPI {
    unsafe {
        if pyo3_ffi::PyDateTimeAPI().is_null() {
            pyo3_ffi::PyDateTime_IMPORT()
        }

        &*pyo3_ffi::PyDateTimeAPI()
    }
}

fn py_timezone_from_fixed_offset(offset: FixedOffset) -> PyResult<*mut PyObject> {
    let seconds_offset = offset.local_minus_utc();
    let api = ensure_datetime_api();
    unsafe {
        let prt =
            (api.Delta_FromDelta)(0, seconds_offset as c_int, 0, true as c_int, api.DeltaType);
        let delta = from_ptr_or_err(prt)?;
        Ok(PyTimeZone_FromOffset(delta))
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
