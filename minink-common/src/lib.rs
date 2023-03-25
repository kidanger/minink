use chrono::NaiveDateTime;

#[derive(Debug, PartialEq)]
pub struct LogEntry {
    pub message: String,
    pub hostname: String,
    pub systemd_unit: String,
    pub timestamp: NaiveDateTime,
}
