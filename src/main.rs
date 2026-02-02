#[cfg(not(feature = "ssr"))]
pub fn main() {
	// hydration handled in lib.rs
}
#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
	use axum::Router;
	use kingdomino::app::*;
	use leptos::prelude::*;
	use leptos_axum::*;

	let conf = get_configuration(None).unwrap();
	let addr = conf.leptos_options.site_addr;
	let leptos_options = conf.leptos_options;

	let app = Router::new()
		.leptos_routes(&leptos_options, generate_route_list(App), {
			let leptos_options = leptos_options.clone();
			move || shell(leptos_options.clone())
		})
		.fallback(leptos_axum::file_and_error_handler(shell))
		.with_state(leptos_options);

	let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
	println!("listening on http://{}", &addr);
	axum::serve(listener, app.into_make_service()).await.unwrap();
}
