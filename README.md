# VpnHood Rust Yönetici

Kendi sunucusunu getiren kiracıların (bring your own server, BYOS) VpnHood sunucularını yönetmesi için Rust tabanlı uygulama.

## Durum

İlk aşama: mevcut VpnHood 9.0.856 sunucusunun `/api/agent/` sözleşmesini uygulayan kimlik doğrulamalı erişim yöneticisi (access manager). Bu sürüm son kullanıcı yönetim paneli değildir; Leptos paneli, kullanıcı/rol yönetimi, sunucu grupları, SSH kurulum işleri ve Debian paketi sonraki aşamalardır.

## Mevcut işlevler

- Sunucu kaydı ve sunucuya özel kimlik bilgisi.
- Gerçek VpnHood istemcileriyle uyumlu `vh://` anahtar üretimi ve iptali.
- Oturum açma/okuma/listeleme/kapatma; tekli ve toplu trafik bildirimi.
- Trafik, cihaz ve son kullanma sınırları.
- Her sunucunun yalnızca bir kiracıya ait olması; birleşik dış anahtarlarla kaynak sahipliği.
- PostgreSQL zorunlu satır düzeyi güvenliği (row-level security, RLS).
- Kimlik bilgilerinin SHA-256 özeti; yapılandırma, token sırları ve oturum anahtarlarının AES-256-GCM ile şifrelenmesi.
- Loopback üzerinde Axum servisi; kapsayıcılar arasında doğrulanan sertifikayla HTTPS.

## Çalışma alanı

- `crates/protocol`: .NET JSON biçimleri, anahtar biçimi, istemci kanıtı doğrulaması.
- `crates/manager`: Axum API, PostgreSQL kayıtları ve yerel işletmen komutları.
- `migrations`: veri tabanı şeması ve kiracı politikaları.
- `tests/ConnectionProbe`: gerçek istemciyle uzak sunucu bağlantısı (NullVpnAdapter; kullanıcı trafiği taşımayı sınamaz).
- `tests/ContractProbe`: kardeş `VpnHood-Current` dizinindeki gerçek .NET tipleriyle uçtan uca sözleşme sınaması.
- `scripts`: yalnızca verilen Debian sınama kapsayıcıları için hazırlık betikleri.
- `work`: Git dışında kalan özel sınama verileri ve derleme çıktıları.

## Derleme

Rust sürümü `rust-toolchain.toml`, bağımlılıklar `Cargo.lock` ile sabittir. Kapsayıcıdaki Debian rustup paketi için:

```bash
export CARGO_HOME=/opt/vpnhood-toolchain/cargo
export RUSTUP_HOME=/opt/vpnhood-toolchain/rustup
cargo build --release --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Kurulum ve sözleşme testi yalnızca root erişimli, ayrılmış sınama kapsayıcısında çalıştırılır:

```bash
bash scripts/setup-test-manager.sh
bash scripts/test-contract.sh
```

Sözleşme sınaması .NET SDK 10, PostgreSQL 17 ve kardeş VpnHood-Current kaynağını gerektirir. Testler yeni kiracı kayıtları oluşturur; var olan veri tabanını sıfırlamaz.

## Güvenlik ve sınırlar

Uygulama ayrı `vhmgr` kullanıcısı ve `NOSUPERUSER NOBYPASSRLS` veri tabanı rolüyle çalışır. PostgreSQL bağlantısı UNIX soketinde eş kimlik doğrulaması (peer authentication) kullanır. Ana anahtar yalnızca sahibi tarafından okunabilen 0600 dosyadadır. Sistem yöneticisi komutları yerel işletmen içindir; web yönetim API'si olarak dışarı açılmaz.

Sunucunun gönderdiği kullanım farklarında (delta) tekil rapor kimliği bulunmaz. Mevcut kaynak kodu başarısız bildirimin baytlarını tekrar göndermediğinden kesintilerde eksik sayım olabilir; tam bir kez muhasebe garantisi verilmez. Kota uygulanması rapor aralığına bağlıdır.

ACME henüz uygulanmadığından ilgili yol 404 döndürür. Sunucu başına tekil erişim anahtarları vardır; sunucu grubu yönlendirmesi ve yük dağıtımı henüz uygulanmamıştır. Kalıcı işler, kullanıcı rolleri, denetim kaydı, hız sınırları ve üretim yedekleme/anahtar döndürme süreci tamamlanmadan üretim kullanımı hedeflenmez.

Sınama TLS sertifika otoritesi (CA) yalnızca bu kapsayıcılarda kullanılır, 30 gün geçerlidir; özel anahtarları depoya eklenmez. Sınama IP'leri betikte açıkça belirtilir ve üretim yapılandırması olarak kullanılmaz.
