
use gloo_console::log;
use gloo_net::http::Request;

use wasm_bindgen::prelude::*;


use yew::prelude::*;

use crate::form::FormComponent;

use crate::logtable::LogTable;

use minink_common::LogEntry;
type Result<T> = core::result::Result<T, JsError>;

pub enum Msg {
    SetLogs(Vec<LogEntry>),
    SetHosts(Vec<String>),
    SetServices(String),
    Error(JsError),
}

pub struct App {
    entries: Vec<LogEntry>,
    hosts: Vec<String>,
    services : Option<String>
}

fn get_value_local_storage(in_key : &str) -> Option<String> {
    let window: web_sys::Window = web_sys::window()?;
    let local_storage = window.local_storage().unwrap().unwrap();
    match local_storage.get_item(in_key)
    {
        Ok(value) => value,
        Err(_) => Some("".to_string())
    }
}



impl Component for App {
    type Message = Msg;

    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let hosts = match get_value_local_storage("hosts") {
            Some(x) => x.split(",").map(str::to_string).collect(),
            None => vec![]
        };

        if !hosts.is_empty() 
        {
            let h = hosts.clone();
            ctx.link().send_future(async {
                Msg::SetHosts(h)
            });
        }


        Self {
            entries: vec![],
            hosts: hosts,
            services: None
        }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        let hosts_cb: Callback<Vec<String>> = _ctx.link().callback(|hosts_value: Vec<String>|Msg::SetHosts(hosts_value));
        let services_cb: Callback<String> = _ctx.link().callback(|services_value: String|Msg::SetServices(services_value));

        //let on_clicked = _ctx.link().callback(Msg::ButtonClick);

        html! {
            <>
                <FormComponent host={self.hosts.clone().join(",")} 
                callback_hosts={hosts_cb} 
                callback_services={services_cb}/>

                //<Form hosts={self.hosts.clone()} callback={hosts_cb}/>
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
                let window: web_sys::Window = web_sys::window().unwrap();
                let local_storage = window.local_storage().unwrap().unwrap();
                match local_storage.set_item("hosts", hosts.join(",").as_str()) {
                    Ok(_) => (),
                    Err(_) => println!("Failed to set value")
                };
                self.hosts = hosts;
                self.fetch_logs(ctx);
                true
            },
            Msg::SetServices(services) => {
                self.services = Some(services);
                log!(format!("{:?}", &self.services));

                self.fetch_logs(ctx);
                true
            },
            Msg::Error(e) => {
                log!(e);
                true
            }
        }
    }
}

async fn fetch_logs(hosts: &[String], services : &Option<String>) -> Result<Vec<LogEntry>> {
    let mut allentries = vec![];

    for h in hosts {
        log!(format!("fetching logs from {h}"));

        let request = Request::new(&format!("{h}/api/extract"))
        .query([("services", match  services {
            Some(v) => v,
            None => ""
            
        })]);
        
        let entries: Vec<LogEntry> = request.method(gloo_net::http::Method::GET)
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
        let services = self.services.clone();
        ctx.link().send_future(async move {
            match fetch_logs(&hosts, &services).await {
                Ok(entries) => Msg::SetLogs(entries),
                Err(e) => Msg::Error(e),
            }
        });
    }
}
