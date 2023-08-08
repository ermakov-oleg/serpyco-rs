use speedate::{Date, DateTime, Time};
use uuid::Uuid;

pub(crate) fn uuid_validator(value: &str) -> bool {
    Uuid::parse_str(value).is_ok()
}

pub(crate) fn datetime_validator(item: &str) -> bool {
    DateTime::parse_str(item).is_ok()
}

pub(crate) fn time_validator(item: &str) -> bool {
    Time::parse_str(item).is_ok()
}

pub(crate) fn date_validator(item: &str) -> bool {
    Date::parse_str(item).is_ok()
}

pub(crate) fn decimal_validator(item: &str) -> bool {
    item.parse::<f64>().is_ok()
}
