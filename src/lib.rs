use chrono::NaiveDateTime;

#[derive(Debug)]
pub struct LogEntry {
    pub message: String,
    pub hostname: String,
    pub systemd_unit: String,
    pub timestamp: NaiveDateTime,
}
