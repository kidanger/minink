use anyhow::Result;

use tokio::sync::mpsc::UnboundedReceiver;

use minink_common::{Filter, LogEntry};

#[derive(thiserror::Error, Debug)]
#[error("LogStream closed")]
pub struct ClosedStream {}

pub struct LogStream {
    receiver: UnboundedReceiver<LogEntry>,
    filter: Filter,
}

impl LogStream {
    pub fn new(receiver: UnboundedReceiver<LogEntry>) -> Self {
        let filter = Filter::default();
        Self { receiver, filter }
    }

    pub fn with_filter(self, filter: Filter) -> Self {
        Self {
            receiver: self.receiver,
            filter,
        }
    }

    pub async fn pull_one(&mut self) -> Result<LogEntry, ClosedStream> {
        loop {
            match self.receiver.recv().await {
                Some(entry) => {
                    if self.filter.accept(&entry) {
                        return Ok(entry);
                    }
                }
                None => return Err(ClosedStream {}),
            }
        }
    }
}
