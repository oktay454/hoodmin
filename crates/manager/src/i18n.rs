use axum::{
	extract::{Query, Request},
	http::{HeaderMap, StatusCode, header},
	middleware::Next,
	response::{IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use serde_json::Value;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locale {
	Tr,
	En,
	Ar,
}
impl Locale {
	pub fn parse(value: &str) -> Option<Self> {
		match value {
			"tr" => Some(Self::Tr),
			"en" => Some(Self::En),
			"ar" => Some(Self::Ar),
			_ => None,
		}
	}
	pub fn code(self) -> &'static str {
		match self {
			Self::Tr => "tr",
			Self::En => "en",
			Self::Ar => "ar",
		}
	}
	pub fn direction(self) -> &'static str {
		if self == Self::Ar { "rtl" } else { "ltr" }
	}
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
	Light,
	Dark,
}
impl Theme {
	pub fn parse(value: &str) -> Option<Self> {
		match value {
			"light" => Some(Self::Light),
			"dark" => Some(Self::Dark),
			_ => None,
		}
	}
	pub fn code(self) -> &'static str {
		match self {
			Self::Light => "light",
			Self::Dark => "dark",
		}
	}
}
struct RequestLanguage {
	locale: Locale,
	theme: Theme,
	return_to: String,
}
tokio::task_local! {
	static LANGUAGE: RequestLanguage;
}
pub fn locale() -> Locale {
	LANGUAGE
		.try_with(|value| value.locale)
		.unwrap_or(Locale::Tr)
}
pub fn theme() -> Theme {
	LANGUAGE
		.try_with(|value| value.theme)
		.unwrap_or(Theme::Light)
}
fn cookie_theme(headers: &HeaderMap) -> Theme {
	headers
		.get(header::COOKIE)
		.and_then(|value| value.to_str().ok())
		.and_then(|value| {
			value.split(';').find_map(|part| {
				let (key, value) = part.trim().split_once('=')?;
				(key == "__Host-vhm_theme")
					.then(|| Theme::parse(value))
					.flatten()
			})
		})
		.unwrap_or(Theme::Light)
}
pub fn return_to() -> String {
	LANGUAGE
		.try_with(|value| value.return_to.clone())
		.unwrap_or("/panel".into())
}
fn cookie_locale(headers: &HeaderMap) -> Locale {
	headers
		.get(header::COOKIE)
		.and_then(|value| value.to_str().ok())
		.and_then(|value| {
			value.split(';').find_map(|part| {
				let (key, value) = part.trim().split_once('=')?;
				(key == "__Host-vhm_lang")
					.then(|| Locale::parse(value))
					.flatten()
			})
		})
		.unwrap_or(Locale::Tr)
}
pub fn safe_return_to(value: &str) -> &str {
	let path = value.split('?').next().unwrap_or("");
	if value.len() <= 2048
		&& !value.chars().any(char::is_control)
		&& (safe_management_path(path)
			|| matches!(
				path,
				"/signin"
					| "/panel" | "/panel/servers"
					| "/panel/tokens"
					| "/panel/sessions"
					| "/panel/audit"
					| "/panel/users"
					| "/panel/account"
			)) {
		value
	} else {
		"/panel"
	}
}
fn safe_management_path(path: &str) -> bool {
	if [
		"/manage/profiles",
		"/manage/server-profiles",
		"/manage/integrations",
		"/manage/bots",
		"/reports",
	]
	.contains(&path)
	{
		return true;
	}
	let parts = path.split('/').collect::<Vec<_>>();
	if parts.len() == 4
		&& parts[0].is_empty()
		&& parts[1] == "manage"
		&& parts[2] == "publications"
		&& uuid::Uuid::parse_str(parts[3]).is_ok()
	{
		return true;
	}
	parts.len() == 5
		&& parts[0].is_empty()
		&& parts[1] == "manage"
		&& uuid::Uuid::parse_str(parts[3]).is_ok()
		&& ((parts[2] == "servers"
			&& ["configuration", "certificates", "routing", "installation"].contains(&parts[4]))
			|| (parts[2] == "farms"
				&& ["publication", "certificates", "destinations"].contains(&parts[4]))
			|| (parts[2] == "tokens" && parts[4] == "policy"))
}
pub async fn middleware(request: Request, next: Next) -> Response {
	let language = RequestLanguage {
		locale: cookie_locale(request.headers()),
		theme: cookie_theme(request.headers()),
		return_to: safe_return_to(
			request
				.uri()
				.path_and_query()
				.map(|v| v.as_str())
				.unwrap_or("/panel"),
		)
		.into(),
	};
	LANGUAGE.scope(language, next.run(request)).await
}
#[derive(Deserialize)]
pub struct LanguageQuery {
	lang: String,
	#[serde(default)]
	return_to: String,
}
pub async fn change(Query(query): Query<LanguageQuery>) -> Response {
	let Some(locale) = Locale::parse(&query.lang) else {
		return (
			StatusCode::BAD_REQUEST,
			crate::views::error("Desteklenmeyen dil."),
		)
			.into_response();
	};
	let cookie = format!(
		"__Host-vhm_lang={}; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=31536000",
		locale.code()
	);
	(
		[(header::SET_COOKIE, cookie)],
		Redirect::to(safe_return_to(&query.return_to)),
	)
		.into_response()
}
#[derive(Deserialize)]
pub struct ThemeQuery {
	theme: String,
	#[serde(default)]
	return_to: String,
}
pub async fn change_theme(headers: HeaderMap, Query(query): Query<ThemeQuery>) -> Response {
	let Some(theme) = Theme::parse(&query.theme) else {
		return (
			StatusCode::BAD_REQUEST,
			crate::views::error("Desteklenmeyen tema."),
		)
			.into_response();
	};
	let cookie = format!(
		"__Host-vhm_theme={}; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=31536000",
		theme.code()
	);
	if headers
		.get(header::ACCEPT)
		.and_then(|value| value.to_str().ok())
		== Some("application/json")
	{
		([(header::SET_COOKIE, cookie)], StatusCode::NO_CONTENT).into_response()
	} else {
		(
			[(header::SET_COOKIE, cookie)],
			Redirect::to(safe_return_to(&query.return_to)),
		)
			.into_response()
	}
}
static EN: OnceLock<Value> = OnceLock::new();
static AR: OnceLock<Value> = OnceLock::new();
pub fn tr(text: &str) -> String {
	let messages = match locale() {
		Locale::Tr => return text.into(),
		Locale::En => EN.get_or_init(|| {
			serde_json::from_str(include_str!("../locales/en.json")).expect("English catalog")
		}),
		Locale::Ar => AR.get_or_init(|| {
			serde_json::from_str(include_str!("../locales/ar.json")).expect("Arabic catalog")
		}),
	};
	messages
		.get(text)
		.and_then(Value::as_str)
		.unwrap_or(text)
		.into()
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn return_address_cannot_escape_panel() {
		for value in [
			"https://evil.invalid",
			"//evil.invalid",
			"/panel/../signin",
			"/panel\r\nLocation: x",
			"/manage/servers/secret",
		] {
			assert_eq!(safe_return_to(value), "/panel");
		}
		assert_eq!(
			safe_return_to("/panel/tokens?q=test&p=2"),
			"/panel/tokens?q=test&p=2"
		);
	}
	#[test]
	fn publishing_pages_keep_language_and_theme_return_paths() {
		for path in [
			"/manage/farms/50331fb8-2282-467b-b6a5-fc8a1fea9c71/destinations",
			"/manage/publications/50331fb8-2282-467b-b6a5-fc8a1fea9c71",
		] {
			assert_eq!(safe_return_to(path), path)
		}
		for path in [
			"/manage/publications/not-an-id",
			"/manage/publications/50331fb8-2282-467b-b6a5-fc8a1fea9c71/push",
			"//manage/publications/50331fb8-2282-467b-b6a5-fc8a1fea9c71",
		] {
			assert_eq!(safe_return_to(path), "/panel")
		}
	}
	#[tokio::test]
	async fn concurrent_languages_are_isolated() {
		async fn translated(locale: Locale) -> String {
			LANGUAGE
				.scope(
					RequestLanguage {
						locale,
						theme: Theme::Light,
						return_to: "/panel".into(),
					},
					async {
						tokio::task::yield_now().await;
						tr("Sunucular")
					},
				)
				.await
		}
		let (en, ar, tr) = tokio::join!(
			translated(Locale::En),
			translated(Locale::Ar),
			translated(Locale::Tr)
		);
		assert_eq!(en, "Servers");
		assert_eq!(ar, "الخوادم");
		assert_eq!(tr, "Sunucular");
	}
	#[test]
	fn catalogs_have_equal_coverage() {
		let en: Value = serde_json::from_str(include_str!("../locales/en.json")).unwrap();
		let ar: Value = serde_json::from_str(include_str!("../locales/ar.json")).unwrap();
		let tr: Value = serde_json::from_str(include_str!("../locales/tr.json")).unwrap();
		assert_eq!(
			en.as_object().unwrap().keys().collect::<Vec<_>>(),
			ar.as_object().unwrap().keys().collect::<Vec<_>>()
		);
		assert_eq!(
			en.as_object().unwrap().keys().collect::<Vec<_>>(),
			tr.as_object().unwrap().keys().collect::<Vec<_>>()
		);
		assert!(
			en.as_object()
				.unwrap()
				.values()
				.chain(ar.as_object().unwrap().values())
				.all(|value| value.as_str().is_some_and(|value| !value.trim().is_empty()))
		);
	}
}
