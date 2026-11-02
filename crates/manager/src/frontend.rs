use axum::{
	extract::Path,
	http::{StatusCode, header},
	response::{IntoResponse, Response},
};
pub fn is_page(path: &str) -> bool {
	path == "/"
		|| path == "/signin"
		|| path == "/panel"
		|| path == "/reports"
		|| path.starts_with("/panel/")
		|| path.starts_with("/manage/")
}
pub fn index() -> Response {
	(
		[(header::CACHE_CONTROL, "no-store")],
		axum::response::Html(include_str!("../assets/index.html")),
	)
		.into_response()
}
pub async fn script() -> Response {
	(
		[
			(header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
			(header::CACHE_CONTROL, "no-cache"),
		],
		include_str!("../assets/spa.js"),
	)
		.into_response()
}
pub async fn locale(Path(locale): Path<String>) -> Response {
	let text = match locale.as_str() {
		"tr.json" => include_str!("../locales/tr.json"),
		"en.json" => include_str!("../locales/en.json"),
		"ar.json" => include_str!("../locales/ar.json"),
		_ => return StatusCode::NOT_FOUND.into_response(),
	};
	(
		[
			(header::CONTENT_TYPE, "application/json; charset=utf-8"),
			(header::CACHE_CONTROL, "no-cache"),
		],
		text,
	)
		.into_response()
}
