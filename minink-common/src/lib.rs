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

fn tokenize(line: &str) -> Vec<String> {
    let tokens: Vec<String> = line
        .split_terminator(|c: char| !c.is_alphanumeric())
        .map(|s| s.to_lowercase())
        .collect();
    tokens
}

impl Filter {
    pub fn accept(&self, entry: &LogEntry) -> bool {
        let matches_patterns = |tokens: &[String], patterns: &[String]| -> bool {
            patterns
                .iter()
                .any(|pattern| tokens.iter().any(|token| token.starts_with(pattern)))
        };

        if let Some(services) = &self.services {
            let entry_service_tokens = tokenize(&entry.service);
            if !matches_patterns(&entry_service_tokens, services) {
                return false;
            }
        }

        if let Some(message_keywords) = &self.message_keywords {
            let entry_message_tokens = tokenize(&entry.message);
            if !matches_patterns(&entry_message_tokens, message_keywords) {
                return false;
            }
        }

        true
    }
}
