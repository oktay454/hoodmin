# VpnHood Manager ile özellik karşılığı

Bu tablo, [VpnHood Manager](https://www.vpnhood.com/vpnhood-manager/) ve Console arayüzünde karşılığı incelenen işlevlerle Hoodmin'in destek kapsamını eşleştirir. Tam ticari ürün eşdeğerliği veya özel yönetim API'sinin aynı olduğu anlamına gelmez. Kullanım için [elkitabını](elkitabi.md), tel sözleşmesi için [protokol başvurusunu](protokol-uyumlulugu.md) kullanın.

## Doğrudan özellikler

| Manager işlevi | Hoodmin karşılığı | Destek sınırı |
|---|---|---|
| Kendi VPN sunucusunu bağlama | Kiracıya ait sunucu kaydı, agent kimliği | Kiracılar arasında ortak sunucu havuzu yok. |
| Linux kullanıcı/parola veya SSH anahtarıyla kurulum | Sabit SSH parmak izli resmî kurucu işi | Genel kabuk/bakım API'si değil. |
| Elle Linux veya appsettings kurulumu | Resmî Linux kurucusu ve JSON ayarı | Mevcut VPN sunucusu JSON ayarıyla bağlanabilir. |
| Access Tokens / Profiles | Toplu anahtar, erişim profili, kota/cihaz/hız/ömür/son tarih | Dönem süresi açık seçilir; kapalı ticari fatura takvimi varsayılmaz. |
| Server Farms / Power / Location | Grup üyeliği, konum/kapasiteye göre yönlendirme | Aynı kiracı; mevcut oturumları canlı taşıma yok. |
| Server Profiles | Ortak VPN ayarları, grup ataması, yeni üyeye ve bağlı gruplara uygulama | Alan bazlı kalıtım yok; son profil kaydı tekil ayarı ezebilir. |
| Advanced Edit | Kaynak sözleşmesindeki 47 yaprak ayar | Çekirdek/yetki etkileri ayrıca doğrulanmalıdır. |
| Farm Token Urls | Şifreli GET, harici PUT/POST/multipart, zamanlama ve durum | Yayım dosyası içerik denetimi; hedef API şablonu işletmene ait. |
| SSL/TLS ve PFX yükleme | Sunucu PFX, ortak grup PFX deposu | Eski anahtarın parmak izi ayrıca yenilenir. |
| Signing / AutoSign | Sunucu başına Let's Encrypt HTTP-01, test/gerçek CA ve yenileme | Her VPN sunucusu kendi FQDN ve yenileme işini kullanır. |
| Usage / Countries | Filtreli CPU/bellek/oturum/hız grafikleri, ülke/cihaz sayısı | Ülke başına trafik veya bağımsız coğrafi konumlandırma iddiası yok. |
| Team ve bot | Beş rol, süreli bot kimliği, JSON API | Yerel hesap; e-posta davet/Google oturumu yok. |
| Erişim kodu ve ödül politikası | Kod özeti, güvenilir sunucu doğrulaması | Hazır reklam ağı veya kapalı callback sözleşmesi değil. |


## Hoodmin işletimi

[Debian paketi](uretim-kurulumu.md), [otomatik yükseltme](yukseltme.md), [yedekleme ve kurtarma](yedekleme.md) ile PostgreSQL satır düzeyi güvenliği (RLS) Hoodmin kurulumuna aittir. [İşletim politikaları](isletim-politikalari.md) ana anahtar döndürmeyi, yedek saklama ve disk eşiğini, HTTPS webhook ve SMTPS bildirimlerini kapsar.
