# Çalışma durumu — 18 Ekim 2026

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

## Ayrı onay bekleyen işlem

Yalnız yeni iki kapsayıcının UID/GID tabanını 100000'den 200000'e taşıma; mevcut ana makine kullanıcılarının alt kimlik aralığından ayırma. Otomatik onay denetimi, kapsayıcı durdurma ve tüm dosya sahipliklerini değiştirme nedeniyle reddetti. Uygulanmadı. Salt okunur plan, tam durdurulmuş kopya/geri dönüş ve son kontrol kapsamı üretim ortamı belgesindedir. İzin gelmeden bu işlem veya dolaylı eşdeğeri yapılmamalıdır.

İlk sahip parolasını yerel dosyaya kopyalama adımı da reddedildi. Parola üretim kapsayıcısında kalır; bu aktarım için ayrıca açık izin olmadan başka yol denenmemelidir. API ve sunucu içindeki mevcut sahip oturumu işlev kontrollerinde kullanılabildi; tarayıcıda güvenilir giriş sayfası doğrulandı.

## Sonraki özellikler

1. Harici POST/PUT şifreli adres yayımı.
2. Grup düzeyinde sunucu profili atama ve otomatik değişiklik yayma; açık seçili toplu uygulama zaten çalışır.
3. Kiracının panelden kendi ACME işini oluşturduğu AutoSign akışı. Mevcut root/Certbot entegrasyonu gerçek sunucuda çalışır.
4. Ana anahtar döndürme, harici hata bildirimi, yedek saklama/disk politikası ve uzun süreli yük/kesinti sınamaları.
5. Masaüstü işletim sistemi VPN rotalarının tümü, Windows kurulum çalıştırması ve kapalı kaynak davranışı gözlenmeyen ticari özellikler ayrıca değerlendirilir.

Eski `yapayzeka`/`vpnhood` ortamları gerileme sınaması için korunur. Kullanıcının eski `vpnhood-test` kaydı değiştirilmedi. Kaynak kardeş `VpnHood-Current` salt okunur kalmıştır. Özel sınama verileri, kimlikler ve kurtarma anahtarları `work/` içinde Git dışındadır; yayımlanmamalıdır.
