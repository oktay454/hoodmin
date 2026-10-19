#[tokio::main]
async fn main() {
	if let Err(error) = hoodmin::run_server().await {
		eprintln!("Hoodmin başlatılamadı: {error}");
		std::process::exit(1);
	}
}
