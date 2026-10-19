# Çalışma durumu — 19 Ekim 2026

Kullanıcı 17 Ekim duraklamasından sonra iki yeni SSH makinesi vererek devam edilmesini istedi. Yeni ortam üretim hedefiyle hazırlanmıştır; tam Console eşdeğerliği ve uzun süreli üretim onayı iddia edilmez.

## Son tamamlanan işler

- Önceki grup adres yenilemesi ve ortak PFX deposu korunur.
- Yeni `hoodmin` ve `vh01` kapsayıcıları, geçerli HTTPS, yeni sahip hesabı ve ilk gerçek VPN sunucusu çalışır.
- Debian paketi; kurulum/yükseltme, geçiş özeti denetimi ve korumalı systemd hizmeti.
- Gerçek ACME alma/yenileme köprüsü, PFX dağıtımı ve CA/TLS denetimi; Certbot zamanlayıcısı etkin.
- Şifreli günlük yedek ve diğer makineye sınırlı SSH kopyası; ayrı veritabanında ana anahtarla kurtarma doğrulandı.
- PFX değişiminde DNS çözümlemesi düzeltildi; 22 Rust testi ve ilgili gerçek sunucu sınamaları geçti.
- Genel sunucuda TCP/UDP/QUIC paket kanallarından UDP DNS ve sunucu TUN/NAT geçişi doğrulandı.

[Üretim ortamı ve erişim](uretim-ortami.md), [kurulum](uretim-kurulumu.md), [kanıtlar](dogrulama.md), [uyumluluk sınırları](protokol-uyumlulugu.md).

## UID/GID ve sahip parolası

Kullanıcı UID/GID taşımasını onayladı, ancak gerekliliğini sordu ve durdurulmuş `lxc-copy` yedeğini önerdi. Değerlendirme sonunda taşıma zorunlu bulunmadı; mevcut 100000 tabanlı eşlemenin korunacağı kullanıcıya açıklandı. Bu işlem uygulanmadı ve bekleyen onay olarak değerlendirilmez. Önemli kapsayıcı değişikliklerinden önce durdurulmuş bağımsız kopya yaklaşımı kullanılacak; bu turda üretim kapsayıcılarında bakım yapılmadı.

İlk sahip parolasını yerel dosyaya kopyalama adımı daha önce reddedildi. Parola üretim kapsayıcısında kalır; bu aktarım için ayrıca açık izin olmadan başka yol denenmemelidir. API ve sunucu içindeki mevcut sahip oturumu işlev kontrollerinde kullanılabildi; tarayıcıda güvenilir giriş sayfası doğrulandı.

## Yeni tamamlanan: harici POST/PUT yayıncısı

Hoodmin'e yönetici onaylı hedef ekleme/düzenleme/kaldırma, tekli/toplu yayım, 5–1440 dakika zamanlama ve panel yeniden başlatmasından sonra toparlanan kalıcı görevler eklendi. Ham metin JSON ile sarılmaz; multipart, URL/başlık/gövde yer tutucuları ve ayrı içerik türü desteklenir. Kimlik ve şablonlar şifreli saklanır; GET'e yükleme kimliği taşınmaz. Doğrulanmış URL yalnız ilgili başlangıç sunucusunun yeni grup anahtarlarına eklenir.

Gerçek HTTPS dosyası .NET ile çözüldü ve hazır VPN sunucusuna gerçek istemci bağlandı. PUT/POST, multipart, bot API, rol/RLS, SSRF ve iş kirası/yeniden başlatma sınamaları geçti. Üç dilde arayüz, yardım pencereleri, `0009_external_publications.sql` ve Debian paket bağımlılığı güncellendi. Eski QA kullanıldı; yeni üretim bu kilometre taşında değiştirilmedi. [Kullanım ve sınırlar](harici-adres-yayimi.md), [Console gözlemi](console-harici-yayim-sinamasi.md).

## Yeni tamamlanan: gruba sunucu profili atama

Aynı profil birden çok gruba atanabilir. Onaylı atama mevcut üyelere, güncel profil yeni ve gruba taşınan sunucuya uygulanır. Profil düzenlemesi onaylandığında bütün bağlı grup üyeleri tek veritabanı işleminde güncellenir. Bir üyede hata bütün işlemi geri alır; üyelik ve profil güncellemesi kiracı düzeyinde sıralanır. Atama kaldırıldığında son ayarlar korunur. Çevrimdışı üyeler normal agent bağlantısında yeni sürümü alır; tüm makinelerde aynı anda uygulama garantisi yoktur. [Kullanım ve API](grup-sunucu-profili.md).

## Yeni tamamlanan: panel AutoSign

Kiracı yöneticisi sunucunun AutoSign ekranından alan adı, iletişim e-postası, test/gerçek CA ve yenileme ayarını yönetir. CA paylaşımı/koşulları ve sertifika etkisi açık onay gerektirir. Certbot root olmadan ayrı görev dizinlerinde çalışır; HTTP-01 kancası önceki dinleyici durumunu geri yükler. Kalıcı işler, kesinti sonrası kira toparlanması, on iki saatte bir kontrol, durum/hata/son tarih ve bot API vardır. [Kullanım ve sınırlar](autosign.md).

Kullanıcı canlı sınamayı gelecekte üretimde kullanacağı ortamlarda yetkilendirdi. Durdurulmuş hoodmin/vh01 kopyaları alındı, panel 0.1.0-6~deb13u1 sürümüne yükseltildi. Test CA doğrulaması gerçek sunucuda geçti ve sertifikayı değiştirmedi; gerçek CA sertifikası alınıp sunucuya uygulandı, dış TLS doğrulandı. Panelin HTTPS sertifikası korundu. Diğer mantıksal/isteğe bağlı geliştirmeler bu çalışmada başlatılmadı.

## Sonraki özellikler

1. Ana anahtar döndürme, harici hata bildirimi, yedek saklama/disk politikası ve uzun süreli yük/kesinti sınamaları.
2. Masaüstü işletim sistemi VPN rotalarının tümü, Windows kurulum çalıştırması ve kapalı kaynak davranışı gözlenmeyen ticari özellikler ayrıca değerlendirilir.

Eski `yapayzeka`/`vpnhood` ortamları gerileme sınaması için korunur. Kullanıcının eski `vpnhood-test` kaydı değiştirilmedi. Kaynak kardeş `VpnHood-Current` salt okunur kalmıştır. Özel sınama verileri, kimlikler ve kurtarma anahtarları `work/` içinde Git dışındadır; yayımlanmamalıdır.
