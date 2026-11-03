# Geliştirme ve doğrulama başvurusu

Bu belge geliştiriciler içindir. Son kullanıcı adımları [elkitabında](elkitabi.md), dağıtım [Debian kurulum kılavuzunda](uretim-kurulumu.md) bulunur.

## Kaynak ve araçlar

- `crates/manager`: Rust uygulaması, statik SPA, JSON veri modelleri, dil sözlükleri ve CLI.
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

`tests/test_runtime_cli.py`, kullanılmayan QA kurulumunda ayrı PostgreSQL kümesi ve HTTPS vekili açar. Uygulama rolü ile yönetici rolünü ayırır, kendi CA/hesabını üretir ve sonunda kaldırır. Mevcut `/etc/hoodmin`, sistem hesabı veya ilk sahip dosyası varsa başlamaz. Sınama için gerekli Debian araçları önceden bulunmalıdır; betik paket kurmaz.

| Araç | Amaç |
|---|---|
| `tests/test_runtime_cli.py` | Yapılandırılmamış başlangıç, doğrulanmış PostgreSQL TLS, yerel peer kimliğiyle anahtar döndürme, CLI/insan/bot API, kesinti/toparlanma ve şifreli dış yedek. |
| `tests/test_operations.py` | Kiracı/rol/CSRF sınırları, agent yapılandırması, sürüm çatışmaları, bütün şifreli tabloların döndürülmesi, kesinti enjeksiyonu, saklama ve bildirim hata/iyileşme geçişleri. Yalnız üstteki betik çağırır. |
| `tests/notification_fixture.py` | Yalnız QA içinde geçici IP/DNS/CA ve bağımsız TLS webhook/SMTPS alıcıları; gerçek TLS doğrulamasıyla teslim. Üst betiğin sonlandırma bloğu geçici ayarları kaldırır. |
| `tests/static_serving.py` | Üstteki izole betikle paket Nginx örnekleri, dahili/statik/ayrı HTTPS sunum, gerçek dosya kopyası, derin bağlantı, 404, durdurulmuş hizmet ve yanlış arka uç sertifika adı. |
| `tests/test_spa.mjs` | Gerçek API'ye bağlı Chromium'da giriş, ekranlar, 47 alan, onay, tema/dil/RTL ve mobil taşma. Node.js'in yerel DevTools protokolü kullanılır; npm bağımlılığı yoktur. |
| `scripts/test-contract.sh`, `tests/ContractProbe` | Hazır .NET türleriyle protokol/anahtar/istemci kanıtı karşılaştırması; ayrıca ayrılmış hedef/veritabanı ister. |
| `tests/ConnectionProbe`, `tests/UdpProbe` | Hazır VPN'e gerçek istemci veri yolu/TCP/UDP/QUIC. |

Tarayıcı sınamasında `HOODMIN_QA_BROWSER_WAIT=500` ile geçici çalışma aralığı açılır. `tests/browser_proxy.py` yalnız loopback üzerinde çalışır ve bütün statik/API isteklerini sabit QA Nginx adresine iletir; QA CA'sını ve `yapayzeka` sertifika adını doğrular. Yerel deneme çerezleri üretim `__Host` çerezleriyle karışmaz. QA sahibini `work/browser-qa/owner.json`, CA'yı `work/browser-qa/ca.crt` altında özel izinle hazırlayın. `chromium --headless --remote-debugging-port=33481 --user-data-dir=...` ve vekil çalışırken `node tests/test_spa.mjs` çalıştırın. Sertifika doğrulamasını kapatan tarayıcı seçenekleri kullanmayın. Bu vekili dağıtım paketine kurmayın.

Canlı CA doğrulaması ayrıca gerçek DNS/dış TCP80, kullanıcı tarafından kabul edilmiş CA koşulları ve ayrılmış hedef gerektirir. Test CA sertifikasının uygulanmadığı, gerçek sertifikanın CA/DNS/PFX eşleşmesi ve yenileme denemesi ayrı denetlenir. Testin başarılı olması uzun süreli üretim yükü veya bağımsız güvenlik denetimi anlamına gelmez.

## Paket ve belge doğrulaması

Paketin sır içermediğini, hizmetin ayrıcalıksız olduğunu, yeni kurulumun otomatik dışa açılmadığını ve geçişlerin ikinci kez çalıştırılabildiğini kontrol edin. Kurtarmayı ayrı veritabanında yedekteki ana anahtarla sınayın. Dil sözlüklerinin anahtarları eşleşmeli; her görünümün kullandığı metin karşılıkları olmalıdır. Markdown bağlantıları hem depoda hem pakette aynı dizin düzenini kullanmalıdır.

Desteklenen protokol sürümü ve bilinmeyen ticari davranışlar [protokol başvurusunda](protokol-uyumlulugu.md) ayrılır. Anlık test sayısı ve sürüm yükseltme tarihçesi kullanıcı kılavuzunun parçası değildir.
