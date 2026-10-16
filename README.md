# Hoodmin

[Türkçe](README.md) · [العربية](README.ar.md) · [English](README.en.md)

Hoodmin, **VpnHood ile uyumlu bağımsız bir sunucu ve erişim yönetim panelidir**. Rust ile geliştirilir; resmî VpnHood Manager ürünü değildir. VpnHood sunucusunu veya istemcisini yeniden yazmaz. Sunucu yazılımı için resmî hazır kurulum kullanılır.

Her kuruluş kendi sunucularını getirir (bring your own server, BYOS). Bir sunucu yalnızca bir kiracıya (tenant) aittir; kiracılar arasında ortak VPN sunucusu havuzu yoktur. Panel VPN trafiğini taşımaz: istemci doğrudan VPN sunucusuna bağlanır.

## Durum

Protokol çekirdeği ve kiracılı panel çalışıyor. VpnHood **9.0.856** ile gerçek .NET sözleşmesi ve uzak istemci bağlantısı doğrulandı. Üretime hazır dağıtım ve kapalı kaynak Manager ile tam özellik eşdeğerliği henüz tamamlanmadı. Çalışma ayrıntıları: [doğrulama kaydı](docs/dogrulama.md).

## Mevcut özellikler

- Resmî Linux/Windows hazır kurulum komutları; SSH parola/özel anahtar ile kurulum ve yeniden kurulum, parmak izi denetimi ve iş günlüğü.
- Kaynak sözleşmesine göre 47 gelişmiş sunucu ayarı; tekrar kullanılabilir sunucu profilleri ve kiracı içinde toplu uygulama.
- Erişim profilleri, ayrı yükleme/indirme hızı, toplam/dönem kotası, ilk kullanımdan başlayan ömür, deneme/ödül planları ve cihaz politikası.
- Grup erişimi, kapasite/güce ve konuma göre yönlendirme, şifreli sunucu adresi yayımı ve erişim kodları.
- Parolalı PFX içe aktarma, sertifika sabitleme ve süreli HTTP-01 doğrulama yanıtı. Otomatik CA alma/yenileme henüz tamamlanmadı.
- Tarih/sunucu filtreli CPU, bellek, oturum ve hız grafikleri; ülkeye göre bildirilen cihaz sayıları. Anahtar açıklaması, müşteri/sipariş/destek kodu ve anahtar indirme.

- Türkçe, Arapça ve İngilizce arayüz; Arapçada sağdan sola düzen (RTL), açık/koyu tema ve güneş/ay düğmesi.
- Genel bakış, sunucular, erişim anahtarları, VPN oturumları, panel kullanıcıları, denetim kaydı ve hesap ayarları.
- Sahip (owner), yönetici (admin), işletmen (operator), erişim anahtarı yazarı (token_writer), gözlemci (viewer) rolleri; kullanıcı ve parola yönetimi, son etkin yönetici koruması.
- Sunucu/grup kaydı; ad, IP/FQDN, TCP portu ve grup düzenleme; bakım modu. Bağlantı kimliği yenilemede açık onay ve eski onayın tekrar kullanımına karşı koruma.
- VpnHood istemcileriyle uyumlu `vh://` erişim anahtarları; işlem başına 1–100 anahtar üretimi, toplu iptal/etkinleştirme/kota/süre güncelleme, arama ve sayfalama.
- Trafik, cihaz ve son kullanma sınırları; oturum sonlandırma kararının sunucu onayına kadar yeniden iletilmesi.
- Canlı özet sayaçları (SSE), sunucu durumu ve CSV kullanım raporu; kısa yardım pencereleri.
- PostgreSQL zorunlu satır düzeyi güvenliği (FORCE RLS), kiracılı dış anahtarlar, yalnız eklenebilir denetim kaydı; Argon2id, AES-256-GCM, CSRF/Origin denetimi ve giriş denemesi sınırlaması.

Botlar için süreli/iptal edilebilir anahtar ve panelle aynı yetki denetimini kullanan JSON yönetim API'si vardır. [API belgesi](docs/yonetim-api.md).

## Mimari ve çalışma alanı

Rust/Tokio/Axum, PostgreSQL/SQLx ve Leptos sunucu taraflı oluşturma (SSR) kullanılır. Küçük JavaScript dosyası etkileşimleri sağlar; tarayıcıda WebAssembly veya hidrasyon kullanılmaz. Nginx HTTPS'i sonlandırır; Rust yalnız loopback üzerinde dinler. VpnHood sunucusu Hoodmin'in `/api/agent/` uç noktalarına bağlanır.

- `crates/manager`: Hoodmin paneli, API ve yerel işletmen komutları; Cargo paket adı `hoodmin`.
- `crates/protocol`: VpnHood JSON/anahtar biçimleri ve istemci kanıtı doğrulaması.
- `crates/paste-compat`: bakımı süren pastey makrosuna yönlendiren uyumluluk katmanı.
- `migrations`: PostgreSQL şeması ve kiracı politikaları.
- `scripts`: ayrılmış Debian sınama kapsayıcıları için kurulum betikleri ve servis dosyaları.
- `tests`: Rust'a ek olarak gerçek HTTPS/PostgreSQL/.NET bütünleşim sınamaları.
- `work`: özel sınama verileri ve çıktılar; Git dışında tutulur. Sırları yayımlamayın.

Ayrıntılar: [mimari](docs/mimari.md), [arayüz ve diller](docs/arayuz-ve-diller.md), [console özellik karşılaştırması](docs/console-karsilastirmasi.md).

## Derleme ve sınama

Doğrulanan ortam Debian 13 / PostgreSQL 17; Rust **1.99.1** araç zinciri `rust-toolchain.toml`, bağımlılıklar `Cargo.lock` ile sabittir. Rust/Cargo, OpenSSL geliştirme kitaplıkları ve yerel derleme araçları gerekir. .NET karşılaştırma sınamaları ayrıca SDK 10 ve kardeş `VpnHood-Current` kaynağını gerektirir; bunlar sınama araçlarıdır, VpnHood sunucusunun kurulum yöntemi değildir.

Sınama kapsayıcısındaki araç zinciri yolları:

```bash
export CARGO_HOME=/opt/vpnhood-toolchain/cargo
export RUSTUP_HOME=/opt/vpnhood-toolchain/rustup
cargo build --release --locked
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo audit --deny warnings
```

`cargo audit` ayrıca kurulu olmalıdır. Son kayıtlarda 20 Rust testi, gerçek HTTPS/PostgreSQL/.NET panel/protokol/profil/rapor sınamaları ve tema/dil HTTPS kontrolleri başarılıdır; bu sonuç genel güvenlik garantisi değildir.

Henüz `.deb` paketi yoktur. `scripts/setup-test-manager.sh` ve `scripts/setup-test-tls.sh` yalnız ayrılmış sınama ortamları içindir; mevcut kurulumda körlemesine yeniden çalıştırılmamalıdır. TLS betiği özel ve kısa ömürlü bir sınama CA'sı oluşturur, üretim sertifika çözümü değildir. Mevcut ters vekil düzeni için [VPS notlarını](docs/vps-ters-vekil.md) okuyun; sertifika doğrulamasını kapatmayın.

İlk hesap yerel `create-account` komutuyla veya `scripts/bootstrap-panel.sh KIRACI_UUID` ile oluşturulur. Betik rastgele başlangıç parolasını `work/panel-admin.txt` içinde 0600 izinleriyle saklar; mevcut parolayı değiştirmez. `scripts/test-contract.sh`, `tests/test_panel.py` ve `tests/test_preferences.py` ortam gereksinimleri için doğrulama kaydına bakın. Bütünleşim sınamaları ayrı kiracılar kullanır.

## Sınırlar ve kalan işler

- Otomatik CA sertifikası alma/yenileme ve gerçek DNS doğrulaması için ayrıca sağlanacak VPN sunucusu bekleniyor; mevcut HTTP-01 yanıtı otomatik sertifika hizmeti değildir.
- Şifreli adres kaynağı HTTPS GET olarak çalışır. Console'daki harici POST/PUT yayım işleri, ortak çiftlik sertifika deposunun tam iş akışı tamamlanmadı.
- `clientPolicies` istemciye duyurulur; ülke bazındaki kapalı kaynak ticari plan kararlarının birebir uygulandığı iddia edilmez. Ödüllü reklam için güvenilir sunucu doğrulayıcısı gerekir; doğrudan reklam ağı entegrasyonu yoktur.
- `.deb`, üretim yedekleme/geri yükleme ve ana anahtar döndürme süreçleri tamamlanmadı. Kurulum işlerinin durumu kalıcıdır; kesilen işler otomatik yeniden çalıştırılmaz.
- Kiracı oluşturma CLI'dadır; herkese açık kayıt/platform yönetim ekranı yoktur.
- Kullanım farkları tekil rapor kimliği taşımaz; kesintilerde tam bir kez muhasebe garantisi yoktur.
- Gerçek TCP/QUIC proxy trafiği, 1 Mbps indirme ve ağ engeli sınandı. İşletim sistemi VPN yönlendirmesi, UDP kullanıcı yükü, sürekli yük ve tüm ayarların her çekirdekte etkisi ayrıca sınanmalıdır.

Güncel alan kataloğu, kanıtlar ve açık işler: [protokol uyumluluğu](docs/protokol-uyumlulugu.md). Console karşılaştırmaları tarihsel inceleme kayıtlarıdır; güncel uygulama durumu için bu belgeyi kullanın.

## Adlandırma ve kurulum uyumluluğu

Ürün ve ana Cargo paketi **Hoodmin** olarak adlandırıldı. Mevcut sınama kurulumunu korumak için çalıştırılabilir dosya/servis adı `vpnhood-manager`, veri tabanı `vpnhood_manager`, sistem kullanıcısı `vhmgr`, `VHM_*` ayarları ve `__Host-vhm_*` çerezleri korunuyor. Depo dizini de mevcut `VpnHood` yolundadır. Bunların Hoodmin adlarına geçişi Debian paketleme sırasında veri ve anahtarları koruyan bir yükseltme olarak ele alınacaktır. VpnHood adı protokol, sunucu ve istemci uyumluluğunu belirtmek için kullanılır.
