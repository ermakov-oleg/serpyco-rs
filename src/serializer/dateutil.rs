use std::os::raw::c_int;

use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, NaiveTime, Offset, Timelike};
use pyo3::{PyErr, PyResult};
use pyo3_ffi::{PyObject, PyTimeZone_FromOffset};

use crate::serializer::types::NONE_PY_TYPE;

use super::encoders::ValidationError;
use super::py::from_ptr_or_err;

pub fn parse_time(value: &str) -> PyResult<*mut PyObject> {
    let time = NaiveTime::parse_from_str(value, "%H:%M:%S%.f")
        .or_else(|_| NaiveTime::parse_from_str(value, "%H:%M"))
        .map_err(|e| InnerParseError::from(e))?;
    let api = ensure_datetime_api();
    let (micros, fold) = chrono_to_micros_and_fold(time);
    unsafe {
        let ptr = (api.Time_FromTimeAndFold)(
            c_int::from(time.hour() as u8),
            c_int::from(time.minute() as u8),
            c_int::from(time.second() as u8),
            micros as c_int,
            NONE_PY_TYPE,
            fold as c_int,
            api.TimeType,
        );
        from_ptr_or_err(ptr)
    }
}

pub fn parse_date(value: &str) -> PyResult<*mut PyObject> {
    let date =
        NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|e| InnerParseError::from(e))?;
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
    let datetime = DateTime::parse_from_rfc3339(value).map_err(|e| InnerParseError::from(e))?;
    let tz = datetime.offset().fix();
    let (micros, fold) = chrono_to_micros_and_fold(datetime);
    let api = ensure_datetime_api();
    let py_tz = py_timezone_from_fixed_offset(tz)?;
    let ptr = unsafe {
        (api.DateTime_FromDateAndTimeAndFold)(
            datetime.year(),
            c_int::from(datetime.month() as u8),
            c_int::from(datetime.day() as u8),
            c_int::from(datetime.hour() as u8),
            c_int::from(datetime.minute() as u8),
            c_int::from(datetime.second() as u8),
            micros as c_int,
            py_tz,
            c_int::from(fold),
            api.DateTimeType,
        )
    };

    from_ptr_or_err(ptr)
}

fn chrono_to_micros_and_fold(time: impl chrono::Timelike) -> (u32, bool) {
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

struct InnerParseError(chrono::ParseError);

impl From<chrono::ParseError> for InnerParseError {
    fn from(other: chrono::ParseError) -> Self {
        Self(other)
    }
}

impl From<InnerParseError> for PyErr {
    fn from(e: InnerParseError) -> Self {
        return ValidationError::new_err(format!("Fail parse datetime {:?}", e.0.to_string()));
    }
}
