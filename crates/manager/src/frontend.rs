use axum::{
	http::{StatusCode, header},
	response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WebSettings {
	pub serve_static: bool,
	pub directory: PathBuf,
}
impl Default for WebSettings {
	fn default() -> Self {
		Self {
			serve_static: true,
			directory: "/usr/share/hoodmin/web".into(),
		}
	}
}
pub fn is_page(path: &str) -> bool {
	let path = path.trim_end_matches('/');
	if [
		"",
		"/signin",
		"/servers",
		"/access-keys",
		"/sessions",
		"/users",
		"/audit",
		"/account",
		"/farms",
		"/profiles",
		"/server-profiles",
		"/bots",
		"/notifications",
		"/integrations",
		"/reports",
	]
	.contains(&path)
	{
		return true;
	}
	let parts: Vec<_> = path.split('/').collect();
	parts.len() == 4
		&& parts[0].is_empty()
		&& uuid::Uuid::parse_str(parts[2]).is_ok()
		&& match parts[1] {
			"servers" => [
				"configuration",
				"routing",
				"certificates",
				"autosign",
				"installation",
				"credential",
			]
			.contains(&parts[3]),
			"farms" => ["server-profile", "certificates", "destinations"].contains(&parts[3]),
			_ => false,
		}
}
pub fn asset(path: &str) -> Option<(&'static str, &'static str)> {
	Some(match path {
		"/assets/spa.js" => ("assets/spa.js", "text/javascript; charset=utf-8"),
		"/assets/app.css" => ("assets/app.css", "text/css; charset=utf-8"),
		"/assets/locales/tr.json" => ("assets/locales/tr.json", "application/json; charset=utf-8"),
		"/assets/locales/en.json" => ("assets/locales/en.json", "application/json; charset=utf-8"),
		"/assets/locales/ar.json" => ("assets/locales/ar.json", "application/json; charset=utf-8"),
		_ => return None,
	})
}
pub async fn serve(settings: &WebSettings, path: &str, head: bool) -> Response {
	if !settings.serve_static {
		return StatusCode::NOT_FOUND.into_response();
	}
	let (file, kind) = match asset(path) {
		Some(asset) => asset,
		None if is_page(path) => ("index.html", "text/html; charset=utf-8"),
		_ => return StatusCode::NOT_FOUND.into_response(),
	};
	match tokio::fs::read(settings.directory.join(file)).await {
		Ok(bytes) => {
			let length = bytes.len().to_string();
			(
				[
					(header::CONTENT_TYPE, kind),
					(header::CONTENT_LENGTH, &length),
				],
				if head { Vec::new() } else { bytes },
			)
				.into_response()
		}
		Err(error) => {
			tracing::error!(%error, file, "Statik arayüz dosyası okunamadı");
			(
				StatusCode::SERVICE_UNAVAILABLE,
				"Arayüz geçici olarak kullanılamıyor.",
			)
				.into_response()
		}
	}
}
#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn routes_exclude_apis_and_arbitrary_files() {
		assert!(is_page("/"));
		assert!(is_page("/servers/"));
		assert!(is_page(
			"/servers/50331fb8-2282-467b-b6a5-fc8a1fea9c71/configuration"
		));
		for path in [
			"/panel",
			"/manage/servers",
			"/api/manage/servers",
			"/s/a/b",
			"/farms/bad/certificates",
			"/assets/missing.js",
			"/etc/passwd",
			"/servers/../signin",
		] {
			assert!(!is_page(path), "{path}");
			assert!(asset(path).is_none(), "{path}");
		}
		assert!(asset("/assets/locales/en.json").is_some());
		assert!(asset("/assets/locales/../../hoodmin.json").is_none());
	}
}
