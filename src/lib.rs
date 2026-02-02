pub mod app;
#[cfg(feature = "ssr")]
pub mod config;
pub mod game;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
	console_error_panic_hook::set_once();
	leptos::mount::hydrate_body(app::App);

	game::create_app().run();
}
