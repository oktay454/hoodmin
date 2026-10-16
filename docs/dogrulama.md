# Çekirdek ve panel doğrulaması — 16 Ekim 2026

## Ortam

- Ana depo: `/home/oktay454/KumHavuzu/YapayZeka/VpnHood`, `main` dalı; uzak Git adresi kullanıcı tarafından eklenecek.
- yapayzeka: Debian 13.7 amd64, 10.0.3.236. vpnhood: Debian 13.7 amd64, 10.0.3.220; lxcbr0 ağı.
- Mevcut VpnHood kaynağı 9.0.856, kardeş VpnHood-Current dizini. Ana makinedeki kaynak değiştirilmedi; derlemeler kapsayıcıdaki kopyasında yapıldı.

## Kurulanlar

- yapayzeka: Debian rustup 1.27.1 ve rsync 3.5.0; Microsoft deposundan .NET SDK 10.0.402 ve ilişkili 10.0.13 çalışma zamanı/hedefleme paketleri.
- Resmî Rust dağıtımından Rust/Cargo 1.99.1 kararlı, rustfmt, Clippy ve wasm32-unknown-unknown standart kitaplığı. Araç zinciri `/opt/vpnhood-toolchain` altında; proje sürümü sabitler. Debian rustc/cargo paketlerinin yerini rustup yönlendirmeleri aldı.
- crates.io üzerinden Cargo Audit 0.22.2 ve Cargo.lock ile sabitlenmiş proje bağımlılıkları. Panel aşamasında Leptos 0.8.22, Argon2 0.5.3, OpenSSL 0.10.81 ve ilişkili Rust bağımlılıkları eklendi; yeni APT paketi gerekmedi, mevcut Debian OpenSSL geliştirme kitaplığı kullanıldı.
- vpnhood: Microsoft deposundan .NET runtime 10.0.13, Debian rsync ve bağımlılıkları.
- PostgreSQL 17, Nginx, derleyici/paketleme araçları önceden mevcuttu.

APT Microsoft deposunun .list/.sources biçiminde iki kez tanımlandığını bildirdi. Kullanıcının kaynak tanımları değiştirilmedi; kurulum engellenmedi.

## Çalışan sınama servisleri

- yapayzeka: `vpnhood-manager.service`, vhmgr kullanıcısı, `127.0.0.1:3080`.
- Nginx: mevcut port 80 yanında `https://yapayzeka:3443`; LXC adresi 10.0.3.236 ve kapsayıcı içi ad çözümü 127.0.1.1 üzerinde dinler. `nginx -t` geçti; Nginx ve Rust servisleri etkin. SSE tamponlaması kapalıdır; vekil kullanım bildirimini kendiliğinden tekrarlamaz.
- vpnhood: özgün gerçek VpnHoodServer.dll ile `vpnhood-test.service` port 2443; panelden üretilen ayrı ayarlarla `vpnhood-panel-test.service` port 2444. Her ikisi Server is ready durumunda.
- Yönetici TLS için 30 günlük özel CA kullanıldı; CA'da kritik BasicConstraints/KeyUsage, sunucu sertifikasında SAN/EKU tanımlı. Python 3.13 katı TLS doğrulaması ve .NET HTTPS başarılı. CA'nın yalnız açık sertifikası vpnhood kapsayıcısına eklendi. TLS doğrulaması kapatılmadı.
- Başlangıç hesabı `admin@vpnhood.test`; parola `work/panel-admin.txt` dosyasında, 0600 ve Git dışında. Dosya ana makinedeki proje dizinine de alındı. Panel parolayı değiştirebilir.

## Otomatik sınamalar

- `cargo fmt --all -- --check`: geçti.
- `cargo test --workspace --locked`: 7 Rust testi geçti. Şifreleme kaynak bağlamı, .NET JSON, bozuk istemci kanıtı, parola/çerez, HTML kaçırma, CSV formül güvenliği ve makro uyumluluğu.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: geçti. Kilitli release derlemesi başarılı.
- `cargo audit --deny warnings`: geçti; 326 kilitli bağımlılıkta RustSec eşleşmesi/uyarısı yok. Bu sonuç genel güvenlik garantisi değildir.
- Kullanılmayan SQLx MySQL/RSA bağımlılıkları önceki aşamada kaldırıldı. Leptos'un bakım dışı paste bağımlılığı için özgün uygulama kaldırılıp bakımı süren pastey 0.2.3 makrosuna yönlendiren küçük yerel uyumluluk katmanı eklendi; uyarı bastırılmadı.
- `scripts/test-contract.sh`: gerçek .NET HttpAccessManager ile yapılandırma, kimliksiz/sahte sunucu reddi, istemci kanıtı, anahtar/kiracı sınırı, süre/iptal, oturum açma/okuma/listeleme/kapatma/yeniden açma, tekli/toplu trafik farkı, negatif kullanım, u64 sınırı ve trafik kotası geçti.
- 16 eşzamanlı istemci isteğinde tek cihaz kotası atomik korundu: yalnız 1 kabul, 15 ret.
- PostgreSQL RLS: bağlamsız sorgu sıfır satır; başka kiracı görünmez; kiracılar arası dış anahtar reddedildi.
- `tests/test_panel.py`: gerçek HTTPS/DB üzerinde giriş/çıkış, çerezler, yanlış parola ve hesap giriş sınırı, CSRF/Origin reddi, üç rol, son yönetici koruması, kullanıcı kapatma/parola değişimi, kiracı sınırı, grup/sunucu oluşturma ve düzenleme, toplu anahtar üretim/iptal/kota/süre, arama/sayfalama, CSV, denetim kaydı ve SSE geçti.
- Panel sınaması gerçek .NET ServerConfig ile `certificates` alanının küçük harfle aktarılmasını, 16 bayt sunucu sırrını ve özel anahtarlı PFX yüklenmesini doğrular. İlk gerçek sunucu denemesinde bulunan alan harfi uyumsuzluğu düzeltildi; gerileme sınaması (regression test) eklendi.
- Panelden oturum kapatma, status yanıtında kararın tekrar gönderilmesi ve sunucu kapanış onayından sonra temizlenmesi geçti. Bakım modu yeni oturumu reddetti. Kimlik yenilemede eski sunucu kimliği 401, yenisi başarılı.
- Uygulama DB rolünün denetim kayıtlarını silme girişimi reddedildi.
- Gerçek VpnHoodClient, hem çekirdeğin hem panelin ürettiği vh:// anahtarla uzak gerçek sunucuya bağlandı. VpnHood'un sertifika eşleşmesi doğrulaması başarılı.

## Görsel doğrulama

Gerçek Leptos SSR çıktıları parola/oturum çerezi taşımadan, CSRF değerleri ayıklanarak geçici yerel önizlemede incelendi. Masaüstü genel bakış/sunucular ile 390×844 dar ekranda genel bakış/anahtar tablosu doğrulandı. Mobil tablo kendi alanında yatay kayar. Gerçek JavaScript ile toplu seçme/temizleme, seçili adet ve forma aktarılacak kimlik sayısı yerel önizlemede doğrulandı. Önizleme panelin dinamik JavaScript davranışını sınamaz. SSE uç noktası ve oturum işlemleri HTTPS/DB sınamalarında denetlendi. Uygulama içi tarayıcı özel CA'ya güvenmediğinden canlı HTTPS tarayıcı sınaması tamamlanmadı; güvenlik uyarısı atlanmadı.

## Sınırlar

ConnectionProbe NullVpnAdapter kullanır; işletim sistemi VPN yönlendirmesini, tünelden kullanıcı trafiğini veya yük altında başarımı sınamaz. Cihaz sınırı yeni oturum kabulünde uygulanır. Kullanım sözleşmesi kesintilerde tam bir kez muhasebe garantisi sağlamaz.

SSH otomasyonu, kalıcı işler, ACME, grup yönlendirmesi/yük dağıtımı, Debian paketleme, üretim yedekleme/anahtar döndürme sonraki aşamadadır. Sınama servisleri ve kayıtlar ayrılmış kapsayıcılarda bırakıldı. Özel anahtarlar, erişim anahtarları ve parolalar `work` altında veya kapsayıcıdaki özel sistem dizinlerindedir; Git'e eklenmez.

## Füme arayüz, yardım ve dil desteği — 16 Ekim 2026

- Son sürüm `https://vhm-test.oktayaktogan.com.tr` üzerinde çalışıyor. CSS ve JavaScript dosyaları HTTPS'ten alınarak kaynaklarıyla SHA-256 üzerinden birebir karşılaştırıldı.
- Clippy uyarısız; çalışma alanında 10 test ve gerçek HTTPS/PostgreSQL/.NET panel sınaması başarılı. Kimlik yenilemede açık onay, eski onayın tekrar reddi, tenant/rol ayrımı ve eski/yeni agent kimlikleri doğrulandı.
- Türkçe, İngilizce ve Arapça tarayıcıda seçildi. Dil ve yön nitelikleri, çeviriler, kullanıcı adlarının değişmemesi ve desteklenmeyen dil hatasının üç dilde gösterimi doğrulandı.
- Arapça masaüstünde menü sağda; sayısal birimler ve e-posta soldan sağa. 390 piksel genişlikte Arapça Sunucular ve Türkçe Genel bakış sayfalarında gövde yatay taşmıyor. Geçici görünüm boyutu sıfırlandı.
- Yardım penceresi açıldı, Escape ile kapatıldı, odak açan düğmeye döndü. VPN adresinin yanındaki yardım da çalışıyor.
- Mevcut Sınama B için yalnız GET onay sayfası açıldı ve Vazgeç ile dönüldü; kimliği değiştirilmedi. Gerçek yenileme sınamaları ayrı test kiracılarına ait sunucularda yapıldı.
- Ekran kanıtları Codex çıktılarında `vhm-fume-panel.jpg`, `vhm-yardim-fume.jpg`, `vhm-kimlik-onayi.jpg`, `vhm-arapca.jpg` ve `vhm-arapca-mobil.jpg`.

Kullanım ve uygulama notları: `docs/arayuz-ve-diller.md`. Yeni paket/bağımlılık kurulmadı.

## Düz renkli açık/koyu temalar — 16 Ekim 2026

- Onaylanan FortiGate/Proxmox/AtlasHub kompozisyonu uygulandı. Genel bakış kısa başlık, dört sayaç, sunucu durum tablosu ve hızlı erişim paneliyle hizalandı.
- Gradyen ve kabartma kaldırıldı. Açık tema sıcak gri/kırık beyaz; koyu tema nötr antrasit. Mor ve ölçülü kehribar vurgu kullanıldı.
- Clippy uyarısız, çalışma alanındaki 10 test başarılı. `tests/test_preferences.py` içinde 18 gerçek HTTPS kontrolü başarılı: iki tema, kalıcı ve güvenli çerez, yönlendirmesiz 204 yanıtı, JavaScript olmadan geçiş, dönüş adresi sınırı, üç dille uyum, geçersiz tercihler ve HTML niteliği güvenliği.
- Tarayıcıda tema anlık değiştirildi; kaydedilmemiş sunucu adı korundu. Deneme metni silindi, herhangi bir kayıt oluşturulmadı. Yeniden yüklemede tercih korundu.
- DOM üzerinden panel/kart/düğme yüzeylerinde `background-image` olmadığı doğrulandı. Son CSS ve JavaScript HTTPS üzerinden SHA-256 ile kaynakla karşılaştırıldı.
- 390 piksel genişlikte Türkçe açık Genel bakış ile Arapça açık/koyu Sunucular görünümünde gövde yatay taşmıyor. Yardım penceresi açık temada çalışıyor. Geçici görünüm boyutu sıfırlandı.
- Tema/dil tercihi dışında mevcut sunucu veya anahtarlar değiştirilmedi. Yeni paket kurulmadı.
- Son ekran kanıtları Codex çıktılarında: `vhm-acik-tema.jpg`, `vhm-koyu-tema.jpg`, `vhm-koyu-arapca-mobil.jpg`. Panel Türkçe/açık tema ile açık bırakıldı.

## Adres düzenleme ve güneş/ay — 16 Ekim 2026

- Clippy uyarısız, çalışma alanındaki 13 test başarılı. Yeni testler IPv4/FQDN/IPv6 geçişini, geçersiz girişte değişmezliği, sertifika/sır korunmasını ve sanal TLS adından gerçek IP'nin ayrılmasını kapsıyor.
- Gerçek HTTPS/PostgreSQL/.NET panel sınaması ayrı kiracılarla başarılı. Adres ve port kaydı, yeni configCode'un agent'a iletilmesi, TCP portları, geçersiz girişte geri alma, başka kiracıya erişim engeli, güncel adresli yeni anahtar ve mevcut sertifika/sır korunumu doğrulandı. Eski erişim anahtarıyla .NET oturumu açılabildi.
- Tarayıcıda mevcut Sınama B yalnız görüntülendi; IP/FQDN `10.0.3.220`, port `2443` ile dolduruluyor. Mevcut kayıt üzerinde Kaydet veya kimlik yenileme yapılmadı.
- Tek tema düğmesinde görünür yazı yok; güneş/ay dönüşüyor, açıklama güncelleniyor, odak korunuyor. SVG genişliği 18 piksel doğrulandı. Son CSS/JavaScript HTTPS üzerinden SHA-256 ile kaynakla eşleştirildi.
- Son ekran kanıtı Codex çıktılarında `vhm-adres-duzenleme.jpg`.

## Hoodmin adı ve üç dilde benioku — 16 Ekim 2026

- Ürün adı Hoodmin; arayüz başlığı, giriş/hata ekranları, HM işareti, alt bilgi, CLI kimliği ve systemd açıklaması güncellendi. Ana Cargo paketi `hoodmin` oldu. Çalıştırılabilir dosya/servis adı ve veri/ayar yolları mevcut kurulumla uyumluluk için korundu; geçiş Debian paketleme aşamasına kaydedildi.
- `README.md` Türkçe, `README.ar.md` Arapça ve `README.en.md` İngilizce olarak aynı kapsamla hazırlandı. UTF-8, kod blokları ve yerel belge bağlantıları kontrol edildi.
- Resmî Manager ürün açıklaması/wiki üzerinden `docs/console-karsilastirmasi.md` hazırlandı. Doğrudan özellikler, Hoodmin'in mantıksal işletim gereksinimleri ve isteğe bağlı kapsam ayrıldı. Console'da yeni bir oturum incelemesi yapılmadı; kapalı kaynak iç mimari hakkında eşdeğerlik iddiası yok.
- `cargo test --workspace --locked`: 13 test geçti. Clippy `--all-targets --locked -- -D warnings` ve kilitli release derlemesi başarılı. Yeni bağımlılık veya sistem paketi eklenmedi; mevcut kilitli bağımlılıklar derlendi.
- Derleme yapayzeka kapsayıcısına dağıtıldı; healthz ve systemd etkinlik kontrolü başarılı. Önceki binary ve servis `work/hoodmin-rename` içinde saklandı. Veri tabanı, şifreleme anahtarı, sunucu kimlikleri ve kayıtlar taşınmadı/değiştirilmedi.
- `tests/test_preferences.py`: 18 HTTPS kontrolü geçti. Türkçe/İngilizce/Arapça giriş sayfalarında Hoodmin/HOODMIN/HM, dil ve RTL/LTR yönü ayrıca doğrulandı.
- Tarayıcıdaki mevcut oturum korunarak genel bakışta `Genel bakış · Hoodmin`, HM işareti ve Hoodmin alt bilgisi doğrulandı. Mevcut çalışma alanının kullanıcı verisi olan adı değiştirilmedi.
- Ekran kanıtı Codex çıktılarında `hoodmin-panel.jpg`.

## Manager pazarlama vaatlerinin incelemesi — 16 Ekim 2026

- Kullanıcının verdiği Manager pazarlama sayfası tarayıcıda okunarak giriş metni ve sekiz özellik kartı `web.rs`, `views.rs`, `management.rs`, agent/protokol ve veri tabanı şemasıyla karşılaştırıldı.
- Karşılaştırma belgesine on maddelik mevcut/kısmi/eksik matrisi eklendi. Önceki listede açık olmayan marka özelleştirmesi ve ekip daveti; Windows sunucu kurulum hedefi, çiftlik ağırlıkları/yeni üye katılımı ve kullanıcı sertifikası içe aktarma ayrıntıları kaydedildi.
- Plan tanımlama vaadinden ödeme/faturalama sistemi; genel site bağlantılarından Hoodmin güvenlik denetimi veya kapalı kaynak iç mimari garantisi çıkarılmadı.
- Bu tur yalnız belgeler güncellendi; uygulama, servis ve kullanıcı verileri değiştirilmedi. Belge bağlantıları ve Git boşluk denetimi başarılı; kod değişmediğinden derleme/işlev sınamaları tekrarlanmadı.
