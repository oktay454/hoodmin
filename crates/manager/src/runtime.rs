use crate::{config::Settings, store::Store};
use anyhow::Result;
use axum::{
	Router,
	extract::{Request, State},
	http::{StatusCode, header},
	response::{IntoResponse, Response},
	routing::get,
};
use clap::Parser;
use std::{
	path::PathBuf,
	sync::{
		Arc,
		atomic::{AtomicBool, Ordering},
	},
	time::Duration,
};
use tokio::sync::RwLock;
use tower::ServiceExt;

#[derive(Parser)]
#[command(name = "hoodmin", version, about = "Hoodmin VPN yönetim hizmeti")]
struct Args {
	#[arg(long,default_value=crate::config::DEFAULT_FILE)]
	config: PathBuf,
	#[arg(long, default_value = "127.0.0.1:3080")]
	listen: std::net::SocketAddr,
}
#[derive(Clone, Default)]
struct Runtime {
	router: Arc<RwLock<Option<Router>>>,
	ready: Arc<AtomicBool>,
}
pub async fn run() -> Result<()> {
	tracing_subscriber::fmt()
		.with_env_filter("hoodmin=info")
		.init();
	let args = Args::parse();
	let settings = Settings::load(&args.config).unwrap_or_default();
	let listen = if args.listen != "127.0.0.1:3080".parse::<std::net::SocketAddr>()? {
		args.listen
	} else {
		settings.listen
	};
	anyhow::ensure!(
		listen.ip().is_loopback(),
		"Hizmet yalnız loopback üzerinde dinleyebilir."
	);
	let listener = tokio::net::TcpListener::bind(listen).await?;
	let runtime = Runtime::default();
	tokio::spawn(supervise(runtime.clone(), args.config));
	let router = Router::new()
		.route("/assets/spa.js", get(crate::frontend::script))
		.route("/assets/locales/{locale}", get(crate::frontend::locale))
		.route("/healthz", get(|| async { "ok" }))
		.route("/readyz", get(readiness))
		.route("/api/status", get(readiness))
		.route(
			"/assets/app.css",
			get(|| async {
				(
					[(header::CONTENT_TYPE, "text/css; charset=utf-8")],
					include_str!("../assets/app.css"),
				)
			}),
		)
		.fallback(dispatch)
		.with_state(runtime)
		.layer(axum::middleware::from_fn(crate::web::security));
	tracing::info!(%listen,"Hoodmin HTTP hizmeti hazır");
	axum::serve(listener, router)
		.with_graceful_shutdown(async {
			let mut terminate =
				tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
					.expect("SIGTERM");
			tokio::select! { _=tokio::signal::ctrl_c()=>{},_=terminate.recv()=>{} }
		})
		.await?;
	Ok(())
}
async fn readiness(State(state): State<Runtime>) -> Response {
	let ready = state.ready.load(Ordering::Acquire);
	(if ready {StatusCode::OK}else{StatusCode::SERVICE_UNAVAILABLE},axum::Json(serde_json::json!({"ready":ready,"message":if ready{"Hazır"}else{"Hizmet geçici olarak kullanılamıyor. Lütfen daha sonra yeniden deneyin."}}))).into_response()
}
async fn dispatch(State(state): State<Runtime>, request: Request) -> Response {
	if request.method() == axum::http::Method::GET && crate::frontend::is_page(request.uri().path())
	{
		return crate::frontend::index();
	}
	if state.ready.load(Ordering::Acquire)
		&& let Some(router) = state.router.read().await.clone()
	{
		return router
			.oneshot(request)
			.await
			.unwrap_or_else(|never| match never {});
	}
	let mut response=(StatusCode::SERVICE_UNAVAILABLE,axum::Json(serde_json::json!({"error":"Hizmet geçici olarak kullanılamıyor. Lütfen daha sonra yeniden deneyin."}))).into_response();
	response.headers_mut().insert(
		header::CACHE_CONTROL,
		header::HeaderValue::from_static("no-store"),
	);
	response
		.headers_mut()
		.insert(header::RETRY_AFTER, header::HeaderValue::from_static("5"));
	response
}
async fn supervise(state: Runtime, path: PathBuf) {
	loop {
		let settings = match Settings::load(&path).and_then(|s| {
			s.validate()?;
			Ok(s)
		}) {
			Ok(s) => s,
			Err(error) => {
				tracing::error!(detail=%crate::config::redact(&error,None),"Hoodmin yapılandırılamadı");
				tokio::time::sleep(Duration::from_secs(5)).await;
				continue;
			}
		};
		match initialize(&settings).await {
			Err(error) => {
				tracing::error!(detail=%crate::config::redact(&error,settings.database_url.as_deref()),"Hoodmin bağımlılıkları hazır değil")
			}
			Ok(store) => {
				let panel = match crate::web::WebState::new(
					store.clone(),
					settings.public_origin.clone(),
				) {
					Ok(p) => p,
					Err(error) => {
						tracing::error!(%error,"Panel hazırlanamadı");
						return;
					}
				};
				*state.router.write().await =
					Some(crate::agent::router(store.clone()).merge(crate::web::router(panel)));
				let notifications = tokio::spawn(crate::notifications::worker(
					store.clone(),
					settings.clone(),
				));
				let publications = tokio::spawn(crate::publication::worker(store.clone()));
				let acme = tokio::spawn(crate::acme::worker(
					store.clone(),
					settings.database_url.clone().unwrap_or_default(),
					settings.master_key_file.clone(),
				));
				loop {
					let result = match tokio::time::timeout(Duration::from_secs(3), async {
						crate::config::check_schema(&store.pool).await?;
						let active: Option<String> = sqlx_core::query_scalar::query_scalar(
							"SELECT active_key_id FROM vault_state WHERE id",
						)
						.fetch_one(&store.pool)
						.await?;
						anyhow::ensure!(
							active.as_deref().is_none_or(|id| id == store.vault.id()),
							"Anahtar değişti; yeniden yükleniyor."
						);
						Ok(())
					})
					.await
					{
						Ok(result) => result,
						Err(error) => Err(error.into()),
					};
					if let Err(error) = &result {
						tracing::error!(detail=%crate::config::redact(error,settings.database_url.as_deref()),"Bağımlılık denetimi başarısız; yeniden hazırlanıyor");
						state.ready.store(false, Ordering::Release);
						break;
					}
					let ready = result.is_ok();
					let previous = state.ready.swap(ready, Ordering::AcqRel);
					if ready && !previous {
						tracing::info!("Hoodmin veritabanı bağlantısı hazır");
					}
					if let Err(error) = result
						&& previous
					{
						tracing::error!(detail=%crate::config::redact(&error,settings.database_url.as_deref()),"Veritabanı bağlantısı kullanılamıyor");
					}
					tokio::time::sleep(Duration::from_secs(5)).await;
					if Settings::load(&path)
						.ok()
						.and_then(|s| serde_json::to_value(s).ok())
						!= serde_json::to_value(&settings).ok()
					{
						break;
					}
				}
				state.ready.store(false, Ordering::Release);
				notifications.abort();
				publications.abort();
				acme.abort();
				*state.router.write().await = None;
				store.pool.close().await;
			}
		}
		tokio::time::sleep(Duration::from_secs(5)).await;
	}
}
async fn initialize(settings: &Settings) -> Result<Store> {
	let vault = settings.vault()?;
	let pool = crate::config::pool(settings.db()?).await?;
	crate::config::check_schema(&pool).await?;
	let vault = crate::rotation::bind(&pool, vault).await?;
	Ok(Store { pool, vault })
}
