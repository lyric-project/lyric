use chrono::{DateTime, Local};

pub const DEFAULT_TIME_FMT: &str = "%Y-%m-%d %H:%M:%S.%f";

/// Current timestamp, in seconds
pub fn current_timestamp() -> i64 {
    Local::now().timestamp()
}

/// Current timestamp, in milliseconds, 1s = 10^3 ms
pub fn current_timestamp_millis() -> i64 {
    Local::now().timestamp_millis()
}

/// Current timestamp, in microseconds, 1ms = 10^3 us
pub fn current_timestamp_micros() -> i64 {
    Local::now().timestamp_micros()
}

pub trait TimeTrait {
    fn as_fmt_str(&self) -> String;
}

impl TimeTrait for DateTime<Local> {
    fn as_fmt_str(&self) -> String {
        self.format(DEFAULT_TIME_FMT).to_string()
    }
}
