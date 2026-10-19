# Hoodmin

[Türkçe](README.md) · [العربية](README.ar.md) · [English](README.en.md)

Hoodmin, Rust ile geliştirilmiş, **VpnHood ile uyumlu bağımsız bir VPN yönetim panelidir**. VPN sunucularını, erişim anahtarlarını, trafik kotalarını, bağlantıları ve sertifikaları tek arayüzden yönetir. Resmî VpnHood Manager ürünü değildir; VPN sunucusu ve istemcisi için resmî VpnHood yazılımları kullanılır.

Her kuruluş kendi VPN sunucusunu getirir (bring your own server, BYOS). Sunucu yalnız bir kiracıya (tenant) aittir. VPN trafiği Hoodmin üzerinden geçmez; istemci doğrudan VPN sunucusuna bağlanır.

## Nereden başlamalı?

1. [Adım adım Debian kurulumu](docs/uretim-kurulumu.md): paket, HTTPS, ilk hesap ve hizmet.
2. [Hoodmin elkitabı](docs/elkitabi.md): ilk VPN sunucusu, erişim anahtarları, gruplar, yetkiler ve günlük kullanım.
3. [AutoSign](docs/autosign.md): VPN sertifikası oluşturma ve yenileme.
4. [Yedekleme ve kurtarma](docs/yedekleme.md): veritabanı, anahtarlar ve sertifikalar.
5. [Sorun giderme](docs/sorun-giderme.md): erişim, çevrimdışı sunucu ve sertifika hataları.

## Kurulum özeti

Debian 13 amd64 üzerinde, root olarak, indirdiğiniz `.deb` dosyasının bulunduğu dizinde:

```bash
apt install ./hoodmin_0.1.0-7~deb13u1_amd64.deb
hoodmin-db-setup
hoodmin-create-owner vpn@example.com https://hoodmin.example.com
```

Ardından [kurulum kılavuzundaki](docs/uretim-kurulumu.md) Nginx/HTTPS adımlarını tamamlayıp `hoodmin.service` hizmetini başlatın. İlk parolanız yalnız kurulum makinesindeki `/root/hoodmin-first-owner.json` dosyasına yazılır. İlk girişten sonra **Hesabım** bölümünden değiştirin. Paket VPN sunucusunu içermez; onu panelin ürettiği resmî kurulum komutuyla ayrı kurarsınız.

## Özellikler

- Türkçe, Arapça ve İngilizce; Arapçada sağdan sola yerleşim (RTL), açık/koyu tema ve bağlamsal yardım.
- Linux için SSH ile veya elle, Windows için PowerShell komutuyla resmî VPN sunucusu kurulumu.
- VPN sunucusu ayarları, tekrar kullanılabilir sunucu profilleri ve grup üyelerine otomatik profil uygulaması.
- Erişim profilleri, toplu `vh://` anahtar üretimi, hız/cihaz/trafik/süre sınırları ve VPN oturumlarını sonlandırma.
- Aynı kiracının grubunda konum/kapasiteye göre yönlendirme; şifreli adres yenileme ve harici HTTPS yayımı.
- PFX yükleme, ortak grup sertifikası ve sunucu başına Let's Encrypt HTTP-01 AutoSign.
- Kullanım grafikleri, bildirilen ülkeye göre cihaz sayıları, CSV dışa aktarımı ve denetim kaydı.
- Beş ekip rolü, süreli bot anahtarları ve JSON yönetim API'si.
- PostgreSQL satır düzeyi güvenlik (RLS), Argon2id parola özeti ve AES-256-GCM ile saklanan yönetim sırları.

## Teknik başvuru

[Mimari](docs/mimari.md) · [API](docs/yonetim-api.md) · [Protokol ve destek sınırları](docs/protokol-uyumlulugu.md) · [Console özellik karşılığı](docs/console-karsilastirmasi.md) · [Geliştirme ve doğrulama](docs/dogrulama.md)

Hizmet hesabı ve PostgreSQL rolü **`vhmgr`**, veritabanı **`vpnhood_manager`** adını kullanır. Bunlar yerel kimlik doğrulaması ve mevcut verilerin sahipliği için sabit teknik adlardır. Ürün adı Hoodmin'dir. Ayrıntılar: [dosyalar ve teknik adlar](docs/uretim-kurulumu.md#dosyalar-ve-teknik-adlar).

## Destek sınırları

Uyumluluk hedefi VpnHood **9.0.856**; paket hedefi Debian 13 amd64'tür. Windows kurucu komutu üretilir; Windows kurulumu uçtan uca sınanmamıştır. AutoSign tek sunucu/tek FQDN ve HTTP-01 içindir; joker alan adı/DNS-01 yoktur. Bulut IP satın alma, başka VPN motorları, faturalama, kiracıya özel markalama ve genel kayıt portalı sunulmaz. Ana anahtar döndürme, harici alarm ve otomatik yedek silme bulunmaz. Uzun süreli yük/kesinti ve tam masaüstü rota sınamaları tamamlanmış değildir; kapalı kaynak Manager ile tam eşdeğerlik iddia edilmez.
