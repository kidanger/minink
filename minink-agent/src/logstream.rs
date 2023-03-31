use anyhow::Result;

use barrage::Receiver;

use minink_common::{Filter, LogEntry};

#[derive(Clone)]
pub struct LogStream {
    receiver: Receiver<LogEntry>,
    filter: Filter,
}

impl LogStream {
    pub fn new(receiver: Receiver<LogEntry>) -> Self {
        let filter = Filter::default();
        Self { receiver, filter }
    }

    pub fn with_filter(self, filter: Filter) -> Self {
        Self {
            receiver: self.receiver,
            filter,
        }
    }

    pub async fn pull_one(&mut self) -> Result<LogEntry> {
        loop {
            match self.receiver.recv_async().await {
                Ok(entry) => {
                    if self.filter.accept(&entry) {
                        return Ok(entry);
                    }
                }
                Err(e) => return Err(anyhow::format_err!("{:?}", e)),
            }
        }
    }
}
