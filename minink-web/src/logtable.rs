use gloo_console::log;
use yew::prelude::*;

use minink_common::LogEntry;

#[derive(Properties, PartialEq)]
pub struct LogTableProps {
    pub entries: Vec<LogEntry>,
}

#[function_component(LogTable)]
pub fn logtable(LogTableProps { entries }: &LogTableProps) -> Html {
    log!("rendering the log table");
    html! {
        <table id="loglist" class="loglist">
            <thead>
                <tr>
                    <th style="width: 180px">
                        { "Date" }
                    </th>
                    <th style="width: 100px">
                        { "Host" }
                    </th>
                    <th style="width: 10%">
                        { "Service" }
                    </th>
                    <th>
                        { "Content" }
                    </th>
                </tr>
            </thead>
            <tbody id="loglist-body">
                {
                    entries
                    .iter()
                    .map(|entry| {
                        html! {
                            <tr>
                                <td>
                                    { entry.timestamp }
                                </td>
                                <td>
                                    { &entry.hostname }
                                </td>
                                <td>
                                    { &entry.service }
                                </td>
                                <td>
                                    <pre>{ &entry.message }</pre>
                                </td>
                            </tr>
                        }
                    })
                    .collect::<Vec<_>>()
                }
            </tbody>
        </table>
    }
}
