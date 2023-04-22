mod app;
mod form;
mod logtable;

use app::App;

fn main() {
    yew::Renderer::<App>::new().render();
}
