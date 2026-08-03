use wasm_bindgen::prelude::*;
use yew::Renderer;

mod app;

#[wasm_bindgen(start)]
pub fn run_app() {
    console_error_panic_hook::set_once();
    Renderer::<app::App>::new().render();
}
