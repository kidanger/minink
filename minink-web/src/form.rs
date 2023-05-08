use yew::prelude::*;
use gloo_console::log;
use web_sys::{HtmlInputElement};
#[derive(Properties, PartialEq)]
pub struct FormProps {
    pub hosts: Vec<String>,
    pub callback_hosts : Callback<Vec<String>>
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub host: String,
    pub callback_hosts : Callback<Vec<String>>,
    pub callback_services : Callback<String>
}

pub enum Msg {
    UpdateHosts(String),
    UpdateFilterServices(String),
    UpdateFilterKeywords(String),
    UpdateLive(bool),
    Validate
}

pub struct FormComponent {
    current_hosts_value: String,
    current_filter_services_value: String,
    current_filter_keywords_value: String,
    is_live : bool
}
impl Component for FormComponent {
    type Message = Msg;
    type Properties = Props;

    fn create(ctx: &Context<Self>) -> Self {
        Self {
            current_hosts_value : ctx.props().host.clone(),
            current_filter_keywords_value: "".to_string(),
            current_filter_services_value: "".to_string(),
            is_live: false
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::UpdateHosts(value) => {
                self.current_hosts_value = value;
                true
            },
            Msg::Validate => {
                ctx.props().callback_hosts.emit(self.current_hosts_value.split(",").map(str::to_string).collect());
                true
            },
            Msg::UpdateFilterKeywords(value)=> {
                self.current_filter_keywords_value = value;
                true
            },
            Msg::UpdateFilterServices(value)=> {
                self.current_filter_services_value = value;
                ctx.props().callback_services.emit(self.current_filter_services_value.clone());

                true
            },
            Msg::UpdateLive(_)=> {
                self.is_live = !self.is_live;
                log!(format!("{}", self.is_live.clone()));
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        //let theme = &ctx.props().host;
        html! {
            <div>

            <div class="column">

            <div class="row">
                <label class="margin-right" for="hosts">{ "Hosts" }</label>

                <input  type="text" name="hosts" id="hosts" placeholder="https://host1,https://host2"
                value={self.current_hosts_value.clone()} 
                oninput={ctx.link().callback(|e : InputEvent| {
                    let input: HtmlInputElement = e.target_unchecked_into();
                    Msg::UpdateHosts(input.value())})}/>
                <button class="submit" onclick={ctx.link().callback(| _ : MouseEvent| {
                    Msg::Validate})}
                >{"Connect"}</button>
            </div>

            <div class="row">
            <label for="live"> { "LIVE" }</label>
            <input type="checkbox" name="live" id="live-button" 
            checked={self.is_live.clone()}
            onclick={ctx.link().callback(|_ : MouseEvent| {
                Msg::UpdateLive(true)})}/>
            </div>

            <div id="filter">
                <div class="filter-input">
                    <div>{"Services"}</div>
                    <input type="text" name="services-filter" id="services-filter" 
                    oninput={ctx.link().callback(|e : InputEvent| {
                        let input: HtmlInputElement = e.target_unchecked_into();
                        Msg::UpdateFilterServices(input.value())})}/>
                </div>
                <div class="filter-input">
                    <div>{ "Message keywords:" }</div>
                    <input type="text" name="message-keywords-filter" id="message-keywords-filter" 
                    oninput={ctx.link().callback(|e : InputEvent| {
                        let input: HtmlInputElement = e.target_unchecked_into();
                        Msg::UpdateFilterKeywords(input.value())})}/>
                </div>
            </div>
            </div>
            </div>
            
            

        }
    }
}



