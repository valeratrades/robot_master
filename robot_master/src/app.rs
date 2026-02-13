use leptos::{html::*, prelude::*};
use leptos_meta::{MetaTags, Title, provide_meta_context};
use leptos_router::{
	components::{A, AProps, Route, Router, Routes},
	path,
};

pub fn shell(options: LeptosOptions) -> impl IntoView {
	view! {
		<!DOCTYPE html>
		<html lang="en">
			<head>
				<meta charset="utf-8" />
				<meta name="viewport" content="width=device-width, initial-scale=1" />
				<AutoReload options=options.clone() />
				<HydrationScripts options />
				<MetaTags />
				<script src="https://cdn.tailwindcss.com"></script>
			</head>
			<body>
				<App />
			</body>
		</html>
	}
}

#[component]
pub fn App() -> impl IntoView {
	provide_meta_context();
	view! {
		<Title text="Robot Master" />
		<Router>
			<TopBar />
			<main class="min-h-screen bg-gray-900">
				<Routes fallback=|| "Not found">
					<Route path=path!("/") view=HomeView />
				</Routes>
			</main>
		</Router>
	}
}

#[component]
fn NavLink(href: &'static str, label: &'static str) -> impl IntoView {
	A(AProps {
		href: href.to_string(),
		children: Box::new(move || span().class("px-3 py-1 rounded hover:bg-gray-700/50 transition-colors").child(label).into_any()),
		target: None,
		exact: false,
		strict_trailing_slash: false,
		scroll: true,
	})
}

#[component]
fn TopBar() -> impl IntoView {
	nav()
		.class("flex items-center px-4 py-2 bg-gray-800 text-white")
		.child(div().class("flex gap-2").child(NavLink(NavLinkProps { href: "/", label: "Home" })))
}

#[component]
fn HomeView() -> impl IntoView {
	section().class("p-4").child(
		div()
			.class("w-full max-w-4xl mx-auto aspect-video bg-black rounded-lg overflow-hidden")
			.child(canvas().id("bevy-canvas").class("w-full h-full")),
	)
}
