use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct FormProps {
    pub hosts: Vec<String>,
}

#[function_component(Form)]
pub fn form(FormProps { hosts }: &FormProps) -> Html {
    let host_str = hosts.join(",");
    html! {
        <>
            <form action="?">
                <label for="hosts">{ "Hosts" }</label>
                <input type="text" name="hosts" id="hosts" placeholder="https://host1,https://host2" value={ host_str }/>
                <input type="submit" class="submit" value="Connect"/>
                <br/>
            </form>
            <br/>

            <input type="checkbox" name="live" id="live-button"/>
            <label for="live"> { "LIVE" }</label>
            <br/>

            <div id="filter">
                <div class="filter-input">
                    <div>{"Services"}</div>
                    <input type="text" name="services-filter" id="services-filter"/>
                </div>
                <div class="filter-input">
                    <div>{ "Message keywords:" }</div>
                    <input type="text" name="message-keywords-filter" id="message-keywords-filter"/>
                </div>
            </div>

        </>
    }
}
