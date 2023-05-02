use anyhow::Result;

use minink_common::{Filter, LogEntry};
use ratatui::widgets::TableState;

pub struct StatefulTable<T> {
    pub state: TableState,
    pub items: Vec<T>,
}

impl<T> StatefulTable<T> {
    pub fn with_items(items: Vec<T>) -> Self {
        Self {
            state: TableState::default(),
            items,
        }
    }

    pub fn next(&mut self) {
        self.state
            .select(match (self.items.len(), self.state.selected()) {
                (0, _) => None,
                (_, None) => Some(0),
                (len, Some(cur)) => {
                    if cur + 1 == len {
                        Some(0)
                    } else {
                        Some(cur + 1)
                    }
                }
            });
    }

    pub fn previous(&mut self) {
        self.state
            .select(match (self.items.len(), self.state.selected()) {
                (0, _) => None,
                (len, None | Some(0)) => Some(len - 1),
                (_, Some(cur)) => Some(cur - 1),
            });
    }
}

pub struct Endpoint {
    pub url: String,
    pub connected: bool,
}

impl Endpoint {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
            connected: false,
        }
    }
}

pub struct App {
    pub endpoints: Vec<Endpoint>,
    pub logs: StatefulTable<LogEntry>,
    pub filter: Filter,
    pub should_quit: bool,
}

impl App {
    pub fn new(endpoints: &[String]) -> Self {
        Self {
            endpoints: endpoints.iter().map(|url| Endpoint::new(url)).collect(),
            logs: StatefulTable::with_items(vec![]),
            filter: Filter::default(),
            should_quit: false,
        }
    }

    pub fn on_up(&mut self) {
        self.logs.previous();
    }

    pub fn on_down(&mut self) {
        self.logs.next();
    }

    pub fn on_right(&mut self) {}

    pub fn on_left(&mut self) {}

    pub async fn on_key(&mut self, c: char) -> Result<()> {
        match c {
            'q' => {
                self.should_quit = true;
            }
            'c' => {
                self.logs.state = TableState::default();
                self.logs.items.clear();
            }
            'r' => {
                self.refresh().await?;
            }
            _ => {}
        }
        Ok(())
    }
}

impl App {
    pub async fn refresh(&mut self) -> Result<()> {
        self.logs.state = TableState::default();
        self.logs.items.clear();
        let filter = &self.filter;
        for e in &mut self.endpoints {
            if !e.connected {
                let client = reqwest::Client::new();
                let res = client
                    .post(format!("{}/api/extract", e.url))
                    .json(filter)
                    .send()
                    .await?
                    .json::<Vec<LogEntry>>()
                    .await?;

                self.logs.items.extend(res);
            }
        }
        Ok(())
    }
}
