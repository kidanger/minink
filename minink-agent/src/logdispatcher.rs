use std::sync::Mutex;

use tokio::sync::mpsc::UnboundedSender;

use minink_common::LogEntry;

use crate::logstream::LogStream;

#[derive(Debug)]
pub struct LogDispatcher {
    senders: Mutex<Vec<UnboundedSender<LogEntry>>>,
}

impl LogDispatcher {
    pub fn new() -> LogDispatcher {
        LogDispatcher {
            senders: Mutex::new(vec![]),
        }
    }

    pub fn send(&self, entry: LogEntry) {
        self.senders
            .lock()
            .unwrap()
            .retain(|sender| sender.send(entry.clone()).is_ok());
    }

    pub fn stream(&self) -> LogStream {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();

        self.senders.lock().unwrap().push(sender);

        LogStream::new(receiver)
    }
}
