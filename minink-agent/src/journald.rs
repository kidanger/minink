use std::sync::Arc;

use anyhow::Result;

use async_process::{Command, Stdio};

use chrono::NaiveDateTime;

use futures_lite::io::BufReader;
use futures_lite::{AsyncBufReadExt, StreamExt};

use serde::Deserialize;

use minink_common::LogEntry;

use crate::logdispatcher::LogDispatcher;

#[derive(Debug)]
pub struct JournaldLogSource {
    dispatcher: Arc<LogDispatcher>,
}

impl JournaldLogSource {
    pub fn new() -> (Self, Arc<LogDispatcher>) {
        let dispatcher = Arc::new(LogDispatcher::new());
        (
            JournaldLogSource {
                dispatcher: dispatcher.clone(),
            },
            dispatcher,
        )
    }

    pub async fn follow(self, since_timestamp: Option<NaiveDateTime>) -> Result<()> {
        let since_format = if let Some(since) = since_timestamp {
            let now = chrono::Utc::now().naive_utc();
            let duration = now - since;
            let ms = duration.num_milliseconds();
            format!("-{}s{}ms", ms / 1000, ms % 1000)
        } else {
            "1 day ago".to_string()
        };

        let mut child = Command::new("journalctl")
            .arg("--follow")
            .arg("--output=json")
            .arg("--output-fields=MESSAGE,_HOSTNAME,_SYSTEMD_UNIT,__REALTIME_TIMESTAMP,SYSLOG_IDENTIFIER,_EXE")
            .arg("--all")
            .arg(&format!("--since={}", since_format))
            .stdout(Stdio::piped())
            .spawn()?;

        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();

        while let Some(line) = lines.next().await {
            let entry = parse_log_entry(&line?)?;
            self.dispatcher.send(entry);
        }

        Err(anyhow::format_err!(
            "journalctl exited: {:?}",
            child.try_status()
        ))
    }
}

#[derive(Debug, Deserialize)]
struct JournaldRawLogEntry {
    #[serde(rename = "MESSAGE")]
    message: JournaldMessage,
    #[serde(rename = "_HOSTNAME")]
    hostname: String,
    #[serde(rename = "_SYSTEMD_UNIT")]
    systemd_unit: Option<String>,
    #[serde(rename = "__REALTIME_TIMESTAMP")]
    timestamp: String,
    #[serde(rename = "SYSLOG_IDENTIFIER")]
    syslog_identifier: Option<String>,
    #[serde(rename = "_EXE")]
    exe: Option<String>,
}

/// see journalctl(1) json format
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum JournaldMessage {
    String(String),
    Bytes(Vec<u8>),
    Multiple(Vec<JournaldMessage>),
}

impl std::fmt::Display for JournaldMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JournaldMessage::String(s) => s.fmt(f),
            JournaldMessage::Bytes(b) => String::from_utf8_lossy(b).fmt(f),
            JournaldMessage::Multiple(messages) => {
                for message in messages {
                    message.fmt(f)?;
                    ";".fmt(f)?;
                }
                Ok(())
            }
        }
    }
}

fn parse_log_entry(line: &str) -> Result<LogEntry> {
    let raw: JournaldRawLogEntry = serde_json::from_str(line)?;
    let timestamp = raw.timestamp.parse()?;
    let timestamp = NaiveDateTime::from_timestamp_micros(timestamp).unwrap();
    let service = raw
        .syslog_identifier
        .or(raw.systemd_unit)
        .or(raw.exe)
        .unwrap_or_default();
    let message = raw.message.to_string();
    Ok(LogEntry {
        message,
        hostname: raw.hostname,
        service,
        timestamp,
    })
}
