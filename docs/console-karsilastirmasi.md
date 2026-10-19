# VpnHood Manager ile özellik karşılığı

Bu tablo, [VpnHood Manager](https://www.vpnhood.com/vpnhood-manager/) ve Console arayüzünde karşılığı incelenen işlevlerle Hoodmin'in destek kapsamını eşleştirir. Tam ticari ürün eşdeğerliği veya özel yönetim API'sinin aynı olduğu anlamına gelmez. Kullanım için [elkitabını](elkitabi.md), tel sözleşmesi için [protokol başvurusunu](protokol-uyumlulugu.md) kullanın.

## Doğrudan özellikler

| Manager işlevi | Hoodmin karşılığı | Destek sınırı |
|---|---|---|
| Kendi VPN sunucusunu bağlama | Kiracıya ait sunucu kaydı, agent kimliği | Kiracılar arasında ortak sunucu havuzu yok. |
| Linux kullanıcı/parola veya SSH anahtarıyla kurulum | Sabit SSH parmak izli resmî kurucu işi | Genel kabuk/bakım API'si değil. |
| Elle Linux/Windows veya appsettings kurulumu | Resmî kurucu komutları ve JSON ayarı | Windows çalıştırması uçtan uca doğrulanmış değil. |
| Access Tokens / Profiles | Toplu anahtar, erişim profili, kota/cihaz/hız/ömür/son tarih | Dönem süresi açık seçilir; kapalı ticari fatura takvimi varsayılmaz. |
| Server Farms / Power / Location | Grup üyeliği, konum/kapasiteye göre yönlendirme | Aynı kiracı; mevcut oturumları canlı taşıma yok. |
| Server Profiles | Ortak VPN ayarları, grup ataması, yeni üyeye ve bağlı gruplara uygulama | Alan bazlı kalıtım yok; son profil kaydı tekil ayarı ezebilir. |
| Advanced Edit | Kaynak sözleşmesindeki 47 yaprak ayar | Çekirdek/yetki etkileri ayrıca doğrulanmalıdır. |
| Farm Token Urls | Şifreli GET, harici PUT/POST/multipart, zamanlama ve durum | Yayım dosyası içerik denetimi; hedef API şablonu işletmene ait. |
| SSL/TLS ve PFX yükleme | Sunucu PFX, ortak grup PFX deposu | Eski anahtarın parmak izi ayrıca yenilenir. |
| Signing / AutoSign | Sunucu başına Let's Encrypt HTTP-01, test/gerçek CA ve yenileme | Tek FQDN; DNS-01/joker alan adı/grup AutoSign yok. DomainOnly ticari kipi birebir sunulmaz. |
| Usage / Countries | Filtreli CPU/bellek/oturum/hız grafikleri, ülke/cihaz sayısı | Ülke başına trafik veya bağımsız coğrafi konumlandırma iddiası yok. |
| Team ve bot | Beş rol, süreli bot kimliği, JSON API | Yerel hesap; e-posta davet/Google oturumu yok. |
| Erişim kodu ve ödül politikası | Kod özeti, güvenilir sunucu doğrulaması | Hazır reklam ağı veya kapalı callback sözleşmesi değil. |

Pazarlamada bulunan bulut IP siparişi ve markalama için kullanılan Console hesabında uygulanabilir yönetim akışı doğrulanmamıştır; Hoodmin bunları sunmaz. Locked IPs kaydından sağlayıcı IP tahsisi veya çalışma etkisi çıkarılmaz. Protokolde karşılığı olmayan ticari davranışlar varsayımla uygulanmaz.

## Mantıksal işletim gereksinimleri

Debian paketi, veritabanı geçişleri, yedekleme/kurtarma ve tenant RLS Hoodmin'in kendi kurulum/işletim gereksinimleridir; Console'da aynı menünün bulunması beklenmez. Ana anahtar döndürme, yedek saklama/disk politikası ve uzun yük/kesinti sınamaları bu sınıftadır; hazır özellik olarak sunulmaz.

## İsteğe bağlı genişletmeler

Harici alarm kanalları ve başka VPN motorları desteği mevcut doğrudan karşılıklar değildir. Hoodmin'de uygulanmış değildir. Ödeme/faturalama, genel kayıt portalı, kiracı markalama veya kiracılar arasında sunucu paylaşımı mevcut kapsamdan çıkarılamaz.
