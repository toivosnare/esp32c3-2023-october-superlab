use chrono::{Datelike, TimeZone, Timelike, Utc};
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct UtcDateTime {
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    nanoseconds: u32,
}

impl From<chrono::DateTime<Utc>> for UtcDateTime {
    fn from(dt: chrono::DateTime<Utc>) -> Self {
        Self {
            year: dt.year(),
            month: dt.month(),
            day: dt.day(),
            hour: dt.hour(),
            minute: dt.minute(),
            second: dt.second(),
            nanoseconds: dt.nanosecond(),
        }
    }
}

impl From<UtcDateTime> for chrono::DateTime<Utc> {
    fn from(value: UtcDateTime) -> Self {
        Utc.with_ymd_and_hms(
            value.year,
            value.month,
            value.day,
            value.hour,
            value.minute,
            value.second,
        )
        .unwrap()
    }
}
