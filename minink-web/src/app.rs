use gloo_console::log;
use gloo_net::http::Request;

use wasm_bindgen::prelude::*;

use web_sys::UrlSearchParams;

use yew::prelude::*;

use crate::form::Form;
use crate::logtable::LogTable;

use minink_common::LogEntry;

type Result<T> = core::result::Result<T, JsError>;

pub enum Msg {
    SetLogs(Vec<LogEntry>),
    SetHosts(Vec<String>),
    Error(JsError),
    Nothing,
}

pub struct App {
    entries: Vec<LogEntry>,
    hosts: Vec<String>,
}

fn parse_hosts() -> Option<Vec<String>> {
    let window = web_sys::window()?;
    let search = window.location().search().ok()?;
    let hosts: Vec<String> = UrlSearchParams::new_with_str(&search)
        .ok()?
        .get("hosts")
        .map(|s| s.split(',').map(|s| s.to_owned()).collect::<Vec<_>>())
        .iter()
        .flatten()
        .map(|host| host.strip_suffix('/').unwrap_or(host))
        .map(|host| host.to_string())
        .collect::<Vec<_>>();
    Some(hosts)
}

impl Component for App {
    type Message = Msg;

    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_future(async {
            match parse_hosts() {
                Some(hosts) => Msg::SetHosts(hosts),
                None => Msg::Nothing,
            }
        });

        Self {
            entries: vec![],
            hosts: vec![],
        }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        html! {
            <>
                <Form hosts={self.hosts.clone()} />
                <LogTable entries={self.entries.clone()} />
            </>
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::SetLogs(logs) => {
                self.entries = logs;
                true
            }
            Msg::SetHosts(hosts) => {
                log!(format!("{:?}", &hosts));
                self.hosts = hosts;
                self.fetch_logs(ctx);
                true
            }
            Msg::Error(e) => {
                log!(e);
                true
            }
            Msg::Nothing => true,
        }
    }
}

async fn fetch_logs(hosts: &[String]) -> Result<Vec<LogEntry>> {
    let mut allentries = vec![];
    for host in hosts {
        log!(format!("fetching logs from {host}"));
        let entries: Vec<LogEntry> = Request::get(&format!("{host}/api/extract"))
            .send()
            .await?
            .json()
            .await?;
        allentries.extend(entries);
    }
    Ok(allentries)
}

impl App {
    fn fetch_logs(&self, ctx: &Context<Self>) {
        let hosts = self.hosts.clone();
        ctx.link().send_future(async move {
            match fetch_logs(&hosts).await {
                Ok(entries) => Msg::SetLogs(entries),
                Err(e) => Msg::Error(e),
            }
        });
    }
}
