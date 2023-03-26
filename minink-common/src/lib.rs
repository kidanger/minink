use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

pub type ServiceName = String;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogEntry {
    pub message: String,
    pub hostname: String,
    pub service: ServiceName,
    pub timestamp: NaiveDateTime,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Filter {
    /// if Some, filter logs with only specific services
    pub services: Option<Vec<ServiceName>>,
    /// if Some, filter logs with that contains one of the keywords in the message
    pub message_keywords: Option<Vec<String>>,
}

impl Filter {
    pub fn accept(&self, entry: &LogEntry) -> bool {
        if let Some(services) = &self.services {
            if !services
                .iter()
                .any(|service| entry.service.contains(service))
            {
                return false;
            }
        }
        if let Some(message_keywords) = &self.message_keywords {
            if !message_keywords
                .iter()
                .any(|keyword| entry.message.contains(keyword))
            {
                return false;
            }
        }
        true
    }
}
