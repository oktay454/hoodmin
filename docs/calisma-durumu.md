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

Gerçek HTTPS dosyası .NET ile çözüldü ve hazır VPN sunucusuna gerçek istemci bağlandı. PUT/POST, multipart, bot API, rol/RLS, SSRF ve iş kirası/yeniden başlatma sınamaları geçti. Üç dilde arayüz, yardım pencereleri, `0009_external_publications.sql` ve Debian paket bağımlılığı güncellendi. Eski QA kullanıldı; yeni üretim bu kilometre taşında değiştirilmez. [Kullanım ve sınırlar](harici-adres-yayimi.md), [Console gözlemi](console-harici-yayim-sinamasi.md).

## Sonraki özellikler

1. Grup düzeyinde sunucu profili atama ve otomatik değişiklik yayma; açık seçili toplu uygulama zaten çalışır.
2. Kiracının panelden kendi ACME işini oluşturduğu AutoSign akışı. Mevcut root/Certbot entegrasyonu gerçek sunucuda çalışır.
3. Ana anahtar döndürme, harici hata bildirimi, yedek saklama/disk politikası ve uzun süreli yük/kesinti sınamaları.
4. Masaüstü işletim sistemi VPN rotalarının tümü, Windows kurulum çalıştırması ve kapalı kaynak davranışı gözlenmeyen ticari özellikler ayrıca değerlendirilir.

Eski `yapayzeka`/`vpnhood` ortamları gerileme sınaması için korunur. Kullanıcının eski `vpnhood-test` kaydı değiştirilmedi. Kaynak kardeş `VpnHood-Current` salt okunur kalmıştır. Özel sınama verileri, kimlikler ve kurtarma anahtarları `work/` içinde Git dışındadır; yayımlanmamalıdır.
