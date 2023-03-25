use anyhow::Result;

use minink_common::{Filter, LogEntry};

use tokio::sync::broadcast;

#[derive(Debug)]
pub struct LogStream {
    receiver: broadcast::Receiver<LogEntry>,
    filter: Filter,
}

impl LogStream {
    pub fn new(receiver: broadcast::Receiver<LogEntry>) -> Self {
        let filter = Filter {
            services: None,
            message_keywords: None,
        };
        Self { receiver, filter }
    }

    pub async fn pull_one(&mut self) -> Result<LogEntry> {
        loop {
            let entry = self.receiver.recv().await?;
            if self.filter.accept(&entry) {
                return Ok(entry);
            }
        }
    }
}
