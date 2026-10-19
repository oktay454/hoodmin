# Geliştirme ve doğrulama başvurusu

Bu belge geliştiriciler içindir. Son kullanıcı adımları [elkitabında](elkitabi.md), dağıtım [Debian kurulum kılavuzunda](uretim-kurulumu.md) bulunur.

## Kaynak ve araçlar

- `crates/manager`: Rust uygulaması, SSR arayüzü, dil sözlükleri ve CLI.
- `crates/protocol`: VpnHood tel sözleşmesi, anahtar biçimleri ve alan kataloğu.
- `crates/paste-compat`: `pastey` kullanan uyumluluk katmanı.
- `migrations`: sıralı PostgreSQL şema geçişleri. **Uygulanmış dosyalar değiştirilmez**; yeni geçiş eklenir.
- `scripts`: paket işletim araçları ve ayrıca ayrılmış QA yardımcıları.
- `tests`: HTTPS/PostgreSQL/.NET bütünleşim ve gerçek sunucu sınamaları.
- `work`: Git dışı özel sınama ve çıktı alanı; sırları depoya veya pakete eklemeyin.

Rust sürümü `rust-toolchain.toml`, bağımlılıklar `Cargo.lock`, Debian gereksinimleri `debian/control` içindedir. Temel doğrulama Debian 13/PostgreSQL 17 içindir. .NET karşılaştırmaları SDK 10 ve salt okunur kardeş `../VpnHood-Current` ağacını kullanır; VPN sunucusunu kaynak koddan kurma yöntemi değildir.

## Derleme ve statik denetim

```bash
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
dpkg-buildpackage -b -us -uc
```

`cargo audit --deny warnings`, ayrıca kurulu `cargo-audit` ve güncel güvenlik veritabanı gerektirir. Bu aracı çalıştırmadan yapılmış bir denetim sonucu varsaymayın. Paket, Rust sınamalarını derleme sırasında çalıştırır; Lintian ile paket çıktısını ayrıca inceleyin.

## Bütünleşim sınamaları

Bunlar **ayrılmış QA ortamı** içindir; üretimde doğrudan çalıştırmayın. Bazıları sunucu ayarlarını/sertifikasını değiştirir, HTTP alıcısı açar veya hizmeti yeniden başlatır. Önce her dosyanın ön koşullarını ve hedefini okuyun.

`tests/panel_support.py` varsayılan olarak QA `vpnhood-manager` ikilisini, `vpnhood_manager` veritabanını ve `vhmgr` rolünü kullanır; paket kurulumunun yollarıyla aynı değildir. HTTPS kökü `VHM_TEST_ORIGIN`, özel CA `VHM_TEST_CA` ile seçilir. Origin denetimi açık kalmalıdır; sertifika denetimini kapatmayın.

| Araç | Amaç |
|---|---|
| `scripts/test-contract.sh`, `tests/ContractProbe` | Gerçek .NET türleriyle protokol/anahtar/istemci kanıtı karşılaştırması. |
| `tests/test_panel.py` | Panel oturumu, roller, kiracı ayrımı, anahtarlar ve kimlik yenileme onayı. |
| `tests/test_preferences.py` | Dil, tema, güvenli çerezler ve yönlendirme sınırları. |
| `tests/test_contract_features.py` | Gelişmiş ayarlar ve erişim sözleşmeleri. |
| `tests/test_farm_rewards.py` | Grup yönlendirmesi, kota/ödül kararları. |
| `tests/test_profiles_reports.py` | Profiller, grafikler, filtreler ve dışa aktarım. |
| `tests/test_team_bots.py` | Ekip/bot rolleri ve JSON yönetim API'si. |
| `tests/test_install_jobs.py` | Ayrılmış hedefe gerçek SSH kurulum işi. |
| `tests/test_native_features.py` | Hazır VPN'de PFX, HTTP-01, hız ve ağ filtresi. |
| `tests/test_farm_publication.py` | Erişilemeyen başlangıçtan grup adresi yenileme. |
| `tests/test_farm_certificates.py` | Grup PFX dağıtımı ve gerçek TLS. |
| `tests/test_external_publications.py` | Ham/multipart yükleme, yöntem geri dönüşü, SSRF, kalıcı görevler. |
| `tests/test_farm_server_profiles.py` | Bağlı grup profili, yeni/taşınan üyeler ve atomik hata. |
| `tests/test_autosign.py` | CA onayı, yetki, iş kirası, native HTTP-01 ve temizlik. |
| `tests/ConnectionProbe`, `tests/UdpProbe` | Hazır VPN'e gerçek istemci veri yolu/TCP/UDP/QUIC. |

Canlı CA doğrulaması ayrıca gerçek DNS/dış TCP80, kullanıcı tarafından kabul edilmiş CA koşulları ve ayrılmış hedef gerektirir. Test CA sertifikasının uygulanmadığı, gerçek sertifikanın CA/DNS/PFX eşleşmesi ve yenileme denemesi ayrı denetlenir. Testin başarılı olması uzun süreli üretim yükü veya bağımsız güvenlik denetimi anlamına gelmez.

## Paket ve belge doğrulaması

Paketin sır içermediğini, hizmetin ayrıcalıksız olduğunu, yeni kurulumun otomatik dışa açılmadığını ve geçişlerin ikinci kez çalıştırılabildiğini kontrol edin. Kurtarmayı ayrı veritabanında yedekteki ana anahtarla sınayın. Dil sözlüklerinin anahtarları eşleşmeli; her görünümün kullandığı metin karşılıkları olmalıdır. Markdown bağlantıları hem depoda hem pakette aynı dizin düzenini kullanmalıdır.

Desteklenen protokol sürümü ve bilinmeyen ticari davranışlar [protokol başvurusunda](protokol-uyumlulugu.md) ayrılır. Anlık test sayısı ve sürüm yükseltme tarihçesi kullanıcı kılavuzunun parçası değildir.
