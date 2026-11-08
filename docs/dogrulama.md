# Geliştirme ve doğrulama başvurusu

Bu belge geliştiriciler içindir. Son kullanıcı adımları [elkitabında](elkitabi.md), dağıtım [Debian kurulum kılavuzunda](uretim-kurulumu.md) bulunur.

## Kaynak ve araçlar

- `crates/manager`: Rust uygulaması, statik SPA, JSON veri modelleri, dil sözlükleri ve CLI.
- `crates/protocol`: VpnHood tel sözleşmesi, anahtar biçimleri ve alan kataloğu.
- `migrations`: sıralı PostgreSQL şema geçişleri. **Uygulanmış dosyalar değiştirilmez**; yeni geçiş eklenir.
- `scripts`: paket lisans envanteri, protokol kataloğu ve bütünleşim sınaması araçları.
- `deploy`: Nginx, systemd ve yapılandırma örnekleri.
- `tests`: HTTPS/PostgreSQL/.NET bütünleşim ve gerçek sunucu sınamaları.
- `work`: Git dışı özel sınama ve çıktı alanı; sırları depoya veya pakete eklemeyin.

Rust sürümü `rust-toolchain.toml`, bağımlılıklar `Cargo.lock`, Debian gereksinimleri `debian/control` içindedir. Derleme için Rust/Cargo, Debian `build-essential`, `debhelper`, `dpkg-dev`, `pkg-config`, `libssl-dev` ve Python 3 gerekir. Temel sınama ortamı Debian 13 ve PostgreSQL 17'dir.

VpnHood protokol karşılaştırmaları .NET 10 SDK ve ayrı bir VpnHood kaynak ağacı kullanır. Varsayılan dizin `../VpnHood-Current` olur. Başka bir konum kullanmak için `VPNHOOD_SOURCE` ortam değişkenine mutlak yolu verin. Hazır VpnHood sunucu dağıtımı, gerçek VPN bağlantı sınamaları için ayrı makinede kurulur.

## Derleme ve statik denetim

```bash
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
dpkg-buildpackage -b -us -uc
```

Rust zinciri rustup ile kurulmuşsa `dpkg` paket bağımlılığı kaydını göremez. Tüm derleme bağımlılıklarını elle doğruladıktan sonra `dpkg-buildpackage -d -b -us -uc` kullanılabilir.

`cargo audit --deny warnings`, ayrıca kurulu `cargo-audit` ve güncel güvenlik veritabanı gerektirir. Bu aracı çalıştırmadan yapılmış bir denetim sonucu varsaymayın. Paket, Rust sınamalarını derleme sırasında çalıştırır; Lintian ile paket çıktısını ayrıca inceleyin.

## Bütünleşim sınamaları

Bunlar **ayrılmış QA ortamı** içindir; üretimde doğrudan çalıştırmayın. Bazıları sunucu ayarlarını/sertifikasını değiştirir, HTTP alıcısı açar veya hizmeti yeniden başlatır. Önce her dosyanın ön koşullarını ve hedefini okuyun.

`tests/test_runtime_cli.py`, kullanılmayan QA kurulumunda ayrı PostgreSQL kümesi ve HTTPS vekili açar. Uygulama rolü ile yönetici rolünü ayırır, HTTPS ve veritabanı için geçici CA'lar ile sınama hesabını oluşturur, sonra kaldırır. Mevcut `/etc/hoodmin`, sistem hesabı veya ilk sahip dosyası varsa başlamaz. Sınama için gerekli Debian araçları önceden bulunmalıdır; betik paket kurmaz.

| Araç | Amaç |
|---|---|
| `tests/test_package_upgrade.py ESKI_DEB YENI_DEB` | Ayrılmış boş Debian kapsayıcısında gerçek dpkg/systemd geçişi; uzak TLS/yerel peer, bağlantı kesintisi, hizmet durumu, kalıcı hata, dosya/PG kilidi ve öldürülen işin doğrulanmış yedekle devamı. |
| `tests/test_runtime_cli.py` | Yapılandırılmamış başlangıç, doğrulanmış PostgreSQL TLS, yerel peer kimliğiyle anahtar döndürme, CLI/insan/bot API, kesinti/toparlanma ve şifreli dış yedek. |
| `tests/test_operations.py` | Kiracı/rol/CSRF sınırları, agent yapılandırması, sürüm çatışmaları, bütün şifreli tabloların döndürülmesi, kesinti enjeksiyonu, saklama ve bildirim hata/iyileşme geçişleri. Yalnız üstteki betik çağırır. |
| `tests/notification_fixture.py` | Yalnız QA içinde geçici IP/DNS/CA ve bağımsız TLS webhook/SMTPS alıcıları; gerçek TLS doğrulamasıyla teslim. Üst betiğin sonlandırma bloğu geçici ayarları kaldırır. |
| `tests/static_serving.py` | Üstteki izole betikle paket Nginx örnekleri, dahili/statik/ayrı HTTPS sunum, gerçek dosya kopyası, derin bağlantı, 404, durdurulmuş hizmet ve yanlış arka uç sertifika adı. |
| `tests/test_upgrade_ui.mjs` | Yerel statik arayüz ve sahte 503 API ile beş yükseltme mesajı; Türkçe/İngilizce/Arapça, RTL ve tek hata kartı. Üretim hesabı gerekmez. |
| `tests/test_spa.mjs` | Gerçek API'ye bağlı Chromium'da giriş, ekranlar, 47 alan, onay, tema/dil/RTL ve mobil taşma. Node.js'in yerel DevTools protokolü kullanılır; npm bağımlılığı yoktur. |
| `scripts/test-contract.sh`, `tests/ContractProbe` | Hazır .NET türleriyle protokol/anahtar/istemci kanıtı karşılaştırması; ayrıca ayrılmış hedef/veritabanı ister. |
| `tests/ConnectionProbe`, `tests/UdpProbe` | Hazır VPN'e gerçek istemci veri yolu/TCP/UDP/QUIC. |

Tarayıcı sınaması için QA makinesinde `HOODMIN_QA_BROWSER_WAIT=500 python3 tests/test_runtime_cli.py` çalıştırın. `BROWSER_READY` satırındaki dizinden `tls/ca.crt` dosyasını ve QA makinesinden `/root/hoodmin-first-owner.json` dosyasını yerel `work/browser-qa/` dizinine sırasıyla `ca.crt` ve `owner.json` olarak alın. Hesap dosyasını 0600 izinle tutun. QA ayrı bir makinedeyse `ssh -N -L 127.0.0.1:3459:127.0.0.1:3459 root@QA_MAKINESI` ile HTTPS tüneli açın. `tests/browser_proxy.py` yalnız loopback üzerinde dinler ve `localhost:3459` adresindeki QA sertifikasını doğrular. Yerelde `python3 tests/browser_proxy.py` ile vekili çalıştırın. Chromium'u `--headless --remote-debugging-port=33481 --user-data-dir=work/browser-qa/chromium` ile başlatıp `node tests/test_spa.mjs` çalıştırın. İşiniz bittiğinde QA çalışma dizinine `browser-finished` işareti koyarak temizliği başlatabilirsiniz.

Canlı CA doğrulaması ayrıca gerçek DNS/dış TCP80, kullanıcı tarafından kabul edilmiş CA koşulları ve ayrılmış hedef gerektirir. Test CA sertifikasının uygulanmadığı, gerçek sertifikanın CA/DNS/PFX eşleşmesi ve yenileme denemesi ayrı denetlenir. Testin başarılı olması uzun süreli üretim yükü veya bağımsız güvenlik denetimi anlamına gelmez.

## Paket ve belge doğrulaması

Paketin sır içermediğini, hizmetin ayrıcalıksız olduğunu, yeni kurulumun otomatik dışa açılmadığını ve geçişlerin ikinci kez çalıştırılabildiğini kontrol edin. Kurtarmayı ayrı veritabanında yedekteki ana anahtarla sınayın. Dil sözlüklerinin anahtarları eşleşmeli; her görünümün kullandığı metin karşılıkları olmalıdır. Markdown bağlantıları hem depoda hem pakette aynı dizin düzenini kullanmalıdır.

## Lisans ve dağıtım

Hoodmin kaynak kodu ve belgeleri GPL-3.0-or-later kapsamındadır. [Lisans metni](../LICENSE). Rust bağımlılıkları kendi lisanslarıyla gelir. Paket derlemesi, Cargo bildirimlerini `dependency-licenses.json` dosyasına ve kilitli bağımlılıkların lisans metinlerini `dependency-notices/` dizinine yerleştirir. Debian sistem kitaplıklarının bildirimleri kendi paketlerinde bulunur.

İkili paket dağıtımında, o pakete karşılık gelen kaynak kodunu ve derleme dosyalarını GPL'nin kaynak sağlama koşullarına uygun biçimde erişilebilir kılın. VpnHood kaynak ve hazır dağıtımı ayrı projeye aittir.
