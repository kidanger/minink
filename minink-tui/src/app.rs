use anyhow::Result;
use futures::{
    stream::{FuturesUnordered, SplitSink, SplitStream},
    StreamExt,
};
use minink_common::{Filter, LogEntry};
use ratatui::widgets::TableState;
use tokio::net::TcpStream;
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};

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

    pub fn following(&self) -> bool {
        let n = self.items.len();
        match self.state.selected() {
            Some(i) => i + 1 == n,
            None => true,
        }
    }

    pub fn push(&mut self, entry: T) {
        let following = self.following();
        self.items.push(entry);
        if following {
            let n = self.items.len();
            self.state.select(Some(n - 1));
        }
    }
}

pub struct EndpointConnection {
    write: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
    read: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
}

pub struct Endpoint {
    pub url: String,
    pub connection: Option<EndpointConnection>,
}

impl Endpoint {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
            connection: None,
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
    pub async fn process_connections(&mut self) -> Result<()> {
        let mut futures = FuturesUnordered::new();
        for e in &mut self.endpoints {
            if let Some(connection) = &mut e.connection {
                let f = connection.read.next();
                futures.push(f);
            }
        }

        if let Some(a) = futures.next().await {
            match a {
                Some(Ok(Message::Text(t))) => {
                    let entry: LogEntry = serde_json::from_str(&t)?;
                    self.logs.push(entry);
                }
                _ => {
                    todo!("{:?}", a)
                }
            }
        } else {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
        Ok(())
    }

    pub async fn refresh(&mut self) -> Result<()> {
        self.logs.state = TableState::default();
        self.logs.items.clear();
        let filter = &self.filter;
        for e in &mut self.endpoints {
            let client = reqwest::Client::new();
            let res = client
                .post(format!("{}/api/extract", e.url))
                .json(filter)
                .send()
                .await?
                .json::<Vec<LogEntry>>()
                .await?;

            self.logs.items.extend(res);

            let ws_url = e.url.replace("http", "ws") + "/ws/live";
            let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url).await?;
            let (write, read) = ws_stream.split();
            e.connection = Some(EndpointConnection { write, read });
        }

        Ok(())
    }
}
