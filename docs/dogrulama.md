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

## Kurulum menüsünün beş yöntemle doğrulanması — 16 Ekim 2026

- Kullanıcının ekran ve örnekleri üzerine açık Console oturumunda Deneme1 → INSTALL incelendi. Parolalı ve anahtarlı SSH formları, Linux komutu, Windows komutu ve appsettings seçeneği tek tek açıldı. Alan/parametre adları doğrulandı; komut/JSON sırları çıktıya veya belgeye alınmadı.
- SSH kimlik bilgileri girilmedi, INSTALL gönderilmedi, yeni sunucu oluşturulmadı ve kimlik yenilenmedi. Sır içeren pencere kapatılarak sunucu ayrıntısı açık bırakıldı.
- Mevcut upstream Linux/Windows kurucu kaynaklarının parametreleri alıp appsettings yazdığı salt okunur kontrol edildi. İlk iki yolun içeride birebir aynı komutu yürüttüğü, kapalı kaynak olduğu ve kurulum başlatılmadığı için kesinleştirilmedi.
- Karşılaştırmada kurulum başlığı kısmi olarak düzeltildi: Hoodmin'de elle bağlantı JSON'u mevcut; eksik işler resmî hazır kurucu için komut üretimi ve SSH yürütme. Yönetim sırrı alanındaki fark ayrıca kaydedildi.
- Bu tur yalnız belgeler güncellendi; uygulama/sınama servisleri değiştirilmedi.

## Doğrudan özelliklerin canlı menü incelemesi — 16 Ekim 2026

- Mevcut Free projede ana menüler ve ayrıntı/formlar incelendi: kurulu sunucu, çiftlik/Power/token URL, sertifika oluşturma/PFX/Signing, erişim profili/kota/hız/ömrü/politika, anahtar üretim/ayrıntı/key, Project/Team/roller/bot formu, Server Profiles/Advanced Edit, Locked IPs ve raporlar.
- Mevcut sunucu kurulumunun REINSTALL düğmesi görüldü; yeniden kurulum çalıştırılmadı. Resmî Linux/Windows güncelleyicilerinin yeni kurucuyu çağırdığı kaynakta okundu; Console yürütme yöntemi doğrulanmadı.
- Marka özelleştirmesi ve sağlayıcıdan IP siparişi ekranları bu hesapta bulunamadı. Mevcut bot olmadığı için anahtar/yetki ayrıntısı için bot oluşturulmadı. Ülke raporunda cihaz sayısı sütunu ve boş sonuç doğrulandı; genel Live Status sayaçları bu incelemede `-` gösterdi.
- Hiçbir kayıt/kimlik/rol/sunucu/sertifika/IP değişmedi; tüm düzenleme formları CANCEL ile kapatıldı. AutoSign yalnız formda seçilip iptal edildi, imzalama başlatılmadı. Sırlar belgeye kaydedilmedi.
- Bulgular `docs/console-menu-incelemesi.md` içinde ekran yolları ve kanıt düzeyleriyle kaydedildi. Kod değişmediği için derleme/işlev sınamaları tekrarlanmadı.
- İnceleme sonunda çiftlik sayfası yeniden yüklendi; sertifikanın Signing değeri Manual olarak kaldığı doğrulandı.

## Kaynak sözleşmesinin uygulanması — 17 Ekim 2026

- `VpnHood-Current` commit `fec557a91908ffe27e238e7e6375a8ead997d2a4`: 26 türün alan kataloğu ve kaynak SHA-256 özetleri çıkarıldı. Kaynak sunucu uygulaması derlenmedi; resmi hazır 9.0.856 kullanıldı.
- 47 gelişmiş ayar, erişim/sunucu profilleri, hız/dönem/ömür/cihaz politikaları, farm yönlendirme, kapasite/konum, şifreli token URL, erişim kodu, SSV ödül doğrulama, PFX/HTTP01, SSH kurulum işi, grafikler/ülke cihaz raporu ve anahtar bilgileri/indirme eklendi.
- `cargo fmt`, `cargo clippy --all-targets --locked -- -D warnings`, 20 Rust testi, `cargo build --release --locked`, 326 bağımlılık için `cargo audit --deny warnings` başarılı. `test_panel`, `test_contract_features`, `test_farm_rewards`, `test_profiles_reports`, `test_preferences` HTTPS/PostgreSQL/.NET ile geçti.
- Gerçek SSH anahtar ve parola+sudo kurulumları tamamlandı. Anahtar uyuşmazlığı bağlantıdan önce reddedildi; iş günlüğünde sır olmadığı ve geçici çalışma dosyalarının silindiği sınandı. Geçici kullanıcı, sudo kuralı, root yetkili anahtarı ve ayrı SSH dinleyicisi kaldırıldı.
- Yeni PFX hazır sunucunun TLS bağlantısındaki sertifikayla birebir karşılaştırıldı. HTTP01 yanıtı gerçek HTTP80 üzerinden doğrulandı; bilinmeyen token 404. Otomatik CA üretimi/yenilemesi bununla tamamlanmış sayılmaz.
- TCP proxy üzerinden 12 MiB ve QUIC üzerinden 256 KiB içerik doğrulandı. 10 MiB başlangıç toleransını aşan son 2 MiB indirme 1 Mbps sınırında 16,72 saniye sürdü. Aynı gerçek hedef sonradan ağ filtresiyle engellendi; yeni hedef bağlantısı kurulmadı. Sınama sonrası HTTP80 ve özel ağ erişimi kapatıldı.
- Debian önceliği korundu: iki kapsayıcıya Microsoft deposundan `libmsquic 2.6.3` ve Debian'dan `libnuma1`; vpnhood kapsayıcısına ayrıca Debian'dan `sudo` kuruldu.
- Yeni ekranlar tarayıcıda Türkçe/İngilizce/Arapça kontrol edildi. JSON düzenleyicilerinin genişliği ve sayfa boşlukları görsel kontrolde düzeltilmiştir.
- Kapsam matrisi ve doğrulanmamış işler `docs/protokol-uyumlulugu.md` içinde ayrıdır. Tam kapalı kaynak Console eşdeğerliği veya üretime hazır paket iddiası yoktur.

## Ekip rolleri ve bot API — 17 Ekim 2026

- Owner/admin/operator/token_writer/viewer yetki sınırları ve son sahip koruması eklendi. Mevcut roller otomatik değiştirilmedi.
- Süreli ve iptal edilebilir bot anahtarları, yalnız özet saklama, parola girişini kapatma, kiracı sınırı ve JSON yönetim API'si uygulandı. API, panelin doğrulama/kilit/denetim işlevlerini paylaşır.
- `test_team_bots.py` geçti; diğer beş bütünleşim dizisi rol/kimlik değişikliğinden sonra yeniden geçti. Clippy, 20 Rust testi ve release derlemesi başarılı.
- Veritabanı ve önceki ikili dosya 0007 şema değişikliğinden önce özel sınama yedeğine alındı. Üç dil çevirileri ve API belgesi eklendi.

## Grup adres yenilemesi ve ortak PFX — 17 Ekim 2026

- Şifreli grup URL'si ulaşılamayan TCP2999 başlangıç adresinden gerçek TCP2550 sunucusuna geçişi sağladı. Gerçek .NET eski sırla yeni hedef belirtecini çözdü; yenilenen anahtarla gerçek VpnHood istemcisi bağlandı. Tam mobil uygulamanın profil ekranı çalıştırılmış sayılmaz.
- Ortak grup PFX deposu mevcut/yeni/gruba taşınan üyeye dağıtıldı; DNS bağlantı adresi ve sunucu sırrı korundu. Yanlış parola 400, onaysız dağıtım 400, gözlemci 403, başka kiracı 404, eski sürüm 409 doğrulandı. Yerel TLS seçicisindeki aynı DNS adı çakışması önlendi. Sınama hedefinin önceki şifreli yapılandırması/grubu geri yüklendi.
- Son derleme: `cargo fmt --check`, uyarısız Clippy, 20 Rust testi ve kilitli release derlemesi başarılı. Panel, sözleşme, farm/ödül, profiller/raporlar, tercihler, ekip/bot, grup yayımı, grup PFX ve native özellikler olmak üzere dokuz bütünleşim dizisi geçti. Katalog yeniden üretildi ve gömülü katalogla aynı bulundu; upstream kaynak ağacı değişmedi.
- Tam native özellik dizisi tekrar geçti: PFX/TLS, gerçek HTTP01/404, TCP 12 MiB, QUIC 256 KiB, 1 Mbps sınırı ve aynı hedefi engelleme. Son 2 MiB indirme 16,70 saniye sürdü. Son yapılandırma uygulanmış; HTTP01 ve özel ağ erişimi kapalı. Kullanıcının eski `vpnhood-test` kaydı değiştirilmedi.
- Grup sertifikası/yayım sayfalarında üç dil ve RTL/LTR HTTPS ile kontrol edildi. Bot ekranı mevcut kullanıcı oturumuyla salt okunur incelendi; ekran kaydı `hoodmin-bot-yonetimi.jpg`. Üç benioku bağlantısı, çeviri anahtarları ve Git boşluk/sır taraması başarılı. Yeni sistem paketi kurulmadı.
- Kullanıcının isteğiyle bu kilometre taşından sonra duraklanır. Harici POST/PUT yayımı ve sonraki işler başlatılmadı; devam noktası [çalışma durumu](calisma-durumu.md).

## Yeni genel sunucular ve Debian paketi — 18 Ekim 2026

- Kullanıcının verdiği iki SSH makinesindeki şablonlardan `hoodmin` ve `vh01` kopyaları oluşturuldu. Ayrıcalıksız eşleme, sabit iç adresler, kapalı nesting, sınırlı TUN aygıtı, kalıcı ağ yönlendirmeleri ve otomatik başlangıç yapılandırıldı. Ana makine kurucusu yeniden çalıştırılmadı; şablonlar korundu.
- `hoodmin.oktayaktogan.com.tr` için gerçek TLS/Nginx/PostgreSQL ve `vpn@oktayaktogan.com.tr` sahip hesabı oluşturuldu. İlk parola yalnız üretimdeki 0600 dosyadadır. Varsayılan sınama hesabı/veritabanı taşınmadı.
- Debian paketi, açık başlangıç komutları, özel ana anahtar, şema özeti/kilidi/işlemleri, systemd korumaları, kılavuz sayfaları ve üç dilde belgeler eklendi. Yeni kurulum ve yükseltme geçti; ana anahtar değişmedi. 22 Rust testi, Clippy, paket derlemesi ve Lintian paket hata/uyarı denetimi başarılı.
- Hazır VpnHood 9.0.856 kuruldu; kaynak sunucu uygulaması derlenmedi. Gerçek VPN alan adı için CA üretimi, HTTP01 köprüsü, geçici dinleyici temizliği, PFX API dağıtımı ve dış CA/TLS karşılaştırması geçti. Gerçek yenileme ayarıyla `--dry-run` başarılı; sınamada dağıtım kancası ayrıca zorlanmadı.
- Sertifika değişimindeki DNS çözümleme bayrağı hatası düzeltildi. İki gerileme sınaması, grup PFX ve tam native PFX/HTTP01/hız/QUIC/ağ filtresi sınaması yeniden geçti. Yeni genel alanda tema/dil HTTPS kontrolleri başarılı.
- Gerçek TCP443 istemci bağlantısı; TCP, UDP ve QUIC paket kanallarından UDP DNS verisi doğrulandı. Sunucu TUN ve NAT sayaçları arttı. Kapsayıcı yeniden başlangıcı sonrasında hizmetler, TLS, agent onayı ve QUIC verisi tekrar geçti.
- `age` şifreli yedek, kaynak IP/komutla sınırlı SSH okuma anahtarı ve günlük ayrı makine kopyası eklendi. İki kopyanın SHA-256 özeti eşleşti. Ayrı veritabanına yükleme ve yedeklenen ana anahtarla yapılandırma çözme geçti; canlı veri üzerine yazılmadı, geçici kurtarma veritabanı kaldırıldı.
- Sonradan UID/GID aralığı taşıması otomatik onay denetimince reddedildi; uygulanmadı. Salt okunur ön inceleme ve bakım/geri dönüş kapsamı belgede hazırdır. İlk sahip parolasının yerel dosyaya kopyalanması da reddedildi; parola sunucuda kaldı. Bu kısıtlar dolaylı yöntemle aşılmadı.
- Paket sürümü, dizinler, başlıca kurulan paketler, zamanlayıcılar, kalan işler ve erişim: [üretim ortamı](uretim-ortami.md).

## POST/PUT yeniden inceleme sınırı — 18 Ekim 2026

Console yayım formunun üç alanı ve dört HTTP kipi salt okunur tekrar doğrulandı; form kaydedilmedi. Geçici dış QA alıcısı otomatik onay denetimince reddedildi. Yeni devam talebi ve eski QA hedefinin farklılığı gösterilerek yapılan tekrar da reddedildi; işlem dolaylı yoldan uygulanmadı. Kullanıcıya açık hedef/kapsam ile ayrıca onay soruldu. Bu aşamada POST/PUT gövdesi, iki yöntemli kipin sırası veya canlı yayım başarısı doğrulanmış sayılmaz.

## Onaylı harici yayım sınaması — 19 Ekim 2026

[Ayrıntılı kayıt](console-harici-yayim-sinamasi.md). İlk yüklemeler VPS ModSecurity JSON çözümleme hatasıyla engellendi; kullanıcı günlüğü kaynağı belirledi. Geçerli JSON dizesi gövdesiyle PUT=405 → POST=201, PUT=201 → POST'un atlanması, başlık/gövde yer tutucuları ve Console OK durumu doğrulandı. URL yer tutucusu değiştirilmeden geldi. Form-Data kodlaması 400 nedeniyle teyit edilmedi. Güvenlik duvarı değiştirilmedi; Hoodmin yayıncısı henüz eklenmedi.

Deneme kayıtları silinip boş Console listesi görüldü. Önceki Nginx dosyası birebir geri alındı; geçici hizmet/port/yol/veriler kaldırıldı. Son durumda yol 404, QA giriş sayfası 200 ve Nginx/panel etkin. Yeni paket kurulmadı, üretim makineleri değiştirilmedi. `tests/publication_sink.py` Python sözdizimi denetiminden geçti.

## ModSecurity sonrası özgün gövde/Form-Data — 19 Ekim 2026

Kullanıcının FQDN'de ModSecurity'yi kapattığını bildirmesinden sonra ham gövde PUT=405 → POST=201 → GET=200 ve Console OK doğrulandı; JSON sarmalama/açma kullanılmadı. Form-Data'nın multipart kodlaması, payload/key alanları, her iki yer tutucunun değişmesi, payload ile yayımlanan dosyanın birebir eşleşmesi ve Console OK de doğrulandı. [İkinci aşama kaydı](console-harici-yayim-sinamasi.md). Tek geçici kayıt ve alıcı temizlendi; Nginx önceki hâline geri getirildi. Ürün yayıncısı henüz eklenmedi; Python sınama aracı için sözdizimi kontrolü geçti.
