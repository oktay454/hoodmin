# VpnHood Rust Yönetici

Kendi sunucusunu getiren kiracıların (bring your own server, BYOS) VpnHood sunucularını yönetmesi için Rust tabanlı uygulama. Bir sunucu yalnızca bir kiracıya (tenant) aittir.

## Durum

Protokol çekirdeği ve kiracılı web paneli çalışıyor. Mevcut VpnHood 9.0.856 sunucusuyla gerçek .NET sözleşmesi ve uzak istemci bağlantısı doğrulandı. Kapalı kaynak VpnHood Manager ile tüm özelliklerde eşdeğerlik henüz tamamlanmadı.

Sınama paneli: **https://yapayzeka:3443**. Kullanıcı `admin@vpnhood.test`; rastgele parola yalnızca proje içindeki `work/panel-admin.txt` dosyasındadır (0600, Git dışında). Sınama CA'sı tarayıcıda güvenilir sayılmayabilir; sertifika doğrulamasını kapatmayın. Üretimde güvenilir TLS sertifikası kullanılmalıdır.

## Mevcut işlevler

- Türkçe, dar ekrana uyumlu Leptos sunucu taraflı oluşturma (server-side rendering, SSR) arayüzü: genel bakış, sunucular, anahtarlar, oturumlar, kullanıcılar, denetim kaydı ve hesap ayarları.
- Yönetici (admin), işletmen (operator), gözlemci (viewer) rolleri; kullanıcı ekleme, rol/durum değiştirme, parola değiştirme ve çıkış. Son etkin yönetici korunur.
- Sunucu/grup kaydı, sunucu adını ve grubunu düzenleme, bakım modu, bağlantı kimliğini yenileme. Panelin verdiği `appsettings.json` sunucuya elle yerleştirilir.
- Gerçek VpnHood istemcileriyle uyumlu `vh://` anahtarlar; işlem başına 1–100 anahtar üretimi, toplu iptal/etkinleştirme/kota/süre güncelleme, arama ve sayfalama.
- Trafik/cihaz/son kullanma sınırları; oturum listeleme ve sunucuya kalıcı kapatma bildirimi. Bildirim sunucu onayına kadar tekrar gönderilir.
- Canlı özet sayaçları (server-sent events, SSE), sunucu durum kartları ve CSV kullanım raporu.
- PostgreSQL zorunlu satır düzeyi güvenliği (row-level security, RLS), kiracı içeren dış anahtarlar ve yalnız eklenebilir denetim kaydı.
- Argon2id parola özetleri; Secure/HttpOnly/SameSite oturum çerezleri, CSRF ve Origin denetimi, giriş denemesi sınırlaması, HTML/CSV enjeksiyonu önlemleri.
- Sunucu kimlikleri ve panel oturumlarının SHA-256 özetleri; yapılandırma, token sırları ve oturum anahtarlarının AES-256-GCM ile şifrelenmesi.

## Çalışma alanı

- `crates/protocol`: .NET JSON biçimleri, anahtar biçimi, istemci kanıtı doğrulaması.
- `crates/manager`: Axum API, Leptos SSR paneli, PostgreSQL kayıtları ve yerel işletmen komutları.
- `crates/paste-compat`: bakım dışı paste yerine pastey makrosunu kullanan geçici uyumluluk katmanı; güvenlik uyarısı bastırmaz.
- `migrations`: veri tabanı şeması ve kiracı politikaları.
- `tests/ConnectionProbe`: gerçek istemciyle uzak sunucu bağlantısı; NullVpnAdapter kullanıcı trafiğini sınamaz.
- `tests/ContractProbe`: kardeş `VpnHood-Current` dizinindeki gerçek .NET tipleriyle sözleşme ve sertifika sınaması.
- `tests/test_panel.py`: gerçek HTTPS/PostgreSQL üzerinde panel, roller ve kiracı ayrımı sınamaları.
- `scripts`: yalnızca verilen Debian sınama kapsayıcıları için hazırlık betikleri.
- `work`: Git dışında özel sınama verileri ve derleme çıktıları.

## Derleme ve sınama

Rust sürümü `rust-toolchain.toml`, bağımlılıklar `Cargo.lock` ile sabittir. Kapsayıcıdaki Debian rustup paketi için:

```bash
export CARGO_HOME=/opt/vpnhood-toolchain/cargo
export RUSTUP_HOME=/opt/vpnhood-toolchain/rustup
cargo build --release --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo audit --deny warnings
```

Kurulum yalnızca ayrılmış, root erişimli sınama kapsayıcısında yapılır:

```bash
bash scripts/setup-test-manager.sh
bash scripts/setup-test-tls.sh
bash scripts/test-contract.sh
python3 tests/test_panel.py
```

.NET sınamaları SDK 10, PostgreSQL 17 ve kardeş `VpnHood-Current` kaynağını gerektirir. Panel sınaması derlenmiş ContractProbe kullanır; önce `test-contract.sh` çalıştırılır. Testler yeni kiracılar oluşturur; mevcut veri tabanını sıfırlamaz. Ayrıntılar `docs/dogrulama.md` içindedir.

İlk panel hesabı, uygun kiracı UUID'si verilerek **bir kez** `bash scripts/bootstrap-panel.sh KIRACI_UUID` ile oluşturulur. Betik mevcut başlangıç parolasını değiştirmez. Yerel CLI `create-account` ve sunucu/anahtar komutları `--help` ile görülebilir.

## Güvenlik ve sınırlar

Uygulama ayrı `vhmgr` kullanıcısı ve `NOSUPERUSER NOBYPASSRLS` veri tabanı rolüyle çalışır. PostgreSQL bağlantısı UNIX soketinde eş kimlik doğrulaması (peer authentication) kullanır. Ana anahtar yalnızca sahibi tarafından okunabilen 0600 dosyadadır. API loopback `127.0.0.1:3080`, Nginx TLS uç noktası `3443` üzerindedir; mevcut port 80 sitesi korunur. Tarayıcı form işlemleri yalnız `VHM_PUBLIC_ORIGIN` adresinden kabul edilir.

Sunucunun kullanım farklarında (delta) tekil rapor kimliği yoktur. Mevcut kaynak kodu başarısız bildirimin baytlarını tekrar göndermediğinden kesintilerde eksik sayım olabilir; tam bir kez muhasebe garantisi verilmez. Kota uygulanması rapor aralığına bağlıdır. Panel parolası 16–128 bayt olmalıdır.

Hesap ve oturum tabloları giriş için küresel kimlik katmanıdır; kiracı üyeliği ve kaynak tabloları RLS ile ayrılır. Web panelinde kiracı kimliği kullanıcıdan alınmaz. Kiracı oluşturma yerel işletmen CLI'sindedir; herkese açık kayıt veya platform yönetim paneli yoktur.

SSH ile otomatik kurulum/bakım, kalıcı iş kuyruğu, grup içi yönlendirme/yük dağıtımı, ACME, Debian paketi ve üretim yedekleme/anahtar döndürme süreci henüz tamamlanmadı. Gruplar şu anda düzenleme amacı taşır; erişim anahtarı tek sunucuya bağlıdır. ACME yolu 404 döndürür. Bu aşama ayrılmış sınama ortamı içindir.

Sınama TLS sertifika otoritesi (CA) 30 gün geçerlidir; özel anahtarları Git'e eklenmez. Sınama IP'leri betikte açıkça belirtilir ve üretim yapılandırması değildir.
