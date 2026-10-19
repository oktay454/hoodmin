#[tokio::main]
async fn main() {
	if let Err(error) = hoodmin::run_cli().await {
		eprintln!("İşlem tamamlanamadı: {error}");
		std::process::exit(1);
	}
}
