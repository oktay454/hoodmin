# Debian 13 üzerinde Hoodmin kurulumu

Bu paket yeni ve ayrılmış kurulum içindir. Eski sınama verilerini kendiliğinden taşımaz. VpnHood sunucusu pakete dahil değildir; resmî hazır kurucudan yüklenir.

## Paket oluşturma

Depodaki kilitli Rust araç zinciri, Cargo bağımlılıkları ve Debian `debhelper`, `dpkg-dev`, `pkg-config`, `libssl-dev` gerekir. `dpkg-buildpackage -b -us -uc` Rust sınamalarını ve release derlemesini çalıştırır, dinamik kitaplık bağımlılıklarını `dh_shlibdeps` ile çıkarır. Rust araç zinciri rustup üzerinden kurulmuşsa dpkg bu derleme bağımlılığını kayıtlı paket olarak göremez; bu ortamda önce Cargo/Rust sürümü ve diğer derleme bağımlılıkları denetlenerek `dpkg-buildpackage -d -b -us -uc` kullanılır. Bu, hedefteki çalışma bağımlılıklarının kontrolünü kaldırmaz. Özel anahtarlar ve `work/` paketlenmez. İlk sürüm yalnız Debian 13 amd64 üzerinde sınanır.

## Yeni kurulum

```bash
apt install ./hoodmin_0.1.0-5~deb13u1_amd64.deb
hoodmin-db-setup
hoodmin-create-owner vpn@example.com https://hoodmin.example.com
systemctl enable --now hoodmin.service
```

Paket kurulumunda sistem hesabı `vhmgr` ve 0600 ana anahtar oluşturulur; hizmet kendiliğinden dışarı açılmaz. Veritabanı `vpnhood_manager` ve `VHM_*` ortam değişkenleri uyumluluk için korunur. İkili dosya `/usr/bin/hoodmin`, hizmet `hoodmin.service`, veri dizini `/var/lib/hoodmin` olur. SSH işi geçici dizini önceki koddaki `/run/vpnhood-manager` ile uyumludur.

`hoodmin-db-setup` PostgreSQL yerel eş (peer) kimliği ve ayrıcalıksız uygulama rolünü kullanır. Şema geçişleri işlem (transaction), danışma kilidi (advisory lock) ve SHA-256 kaydıyla izlenir; uygulanmış geçiş değiştirilirse hata verir. İzlenmeyen eski şema kendiliğinden devralınmaz. Paket yükseltmesinde önce yedek alın, hizmeti durdurun, paketi güncelleyin, geçişleri uygulayıp hizmeti yeniden başlatın.

İlk sahip parolası yalnız root tarafından okunabilen `/root/hoodmin-first-owner.json` dosyasına yazılır. Varsayılan/sınama parolası yoktur. Parolayı güvenli saklayın ve ilk girişten sonra hesap ekranından değiştirin. Dosyayı yayımlamayın. Mevcut hesap varsa başlangıç işlemi tekrarlanmaz.

HTTPS ters vekili aynı kapsayıcıdaki `127.0.0.1:3080` hizmetine bağlanmalıdır. Proxy dış istemciden gelen `X-Forwarded-For` değerini geçirmemeli, gerçek bağlantı adresini yazmalıdır. Yalnız yerel ters vekil güvenilir sayılır. TLS doğrulamasını kapatmayın.

## Yedek ve kurtarma

`hoodmin-backup`, veritabanının özel biçimli dökümünü, ana anahtarı, Hoodmin/Certbot ayarlarını, ortam ayarını ve paket sürümünü 0700 bir dizine yazar. Dosyalar sır içerir; ana makine dışındaki şifreli yedeğe ayrıca aktarılmalıdır. Aynı diskteki yedek disk kaybına karşı koruma sağlamaz. Otomatik silme yapılmaz.

Kurtarma hedefi önce ayrı bir PostgreSQL veritabanı olmalıdır. `pg_restore --exit-on-error --dbname=AYRI_VERITABANI database.dump` ile nesneleri yükleyin; aynı uygulama rolü ve yedeklenen ana anahtar olmadan şifreli sunucu kayıtları kullanılamaz. Canlı veritabanını silen veya üzerine yazan otomatik komut verilmez. Önce ayrı hedefte hesap/sunucu sayıları, RLS, anahtar çözme ve hizmet başlatma kontrol edilir.

## Sertifika

Panel için Debian Certbot ve Nginx kullanılabilir. VPN sertifikası panelden PFX olarak uygulanır; HTTP-01 köprüsü ayrı VPN sunucusundaki doğrulamayı taşıyabilir. ACME istemcisinin ürettiği sertifikayı Hoodmin'e dağıtması ve yenileme zamanlayıcısı ayrıca yapılandırılmalıdır. Yalnız HTTP-01 köprüsünün bulunması otomatik sertifika hizmeti anlamına gelmez.

## Sertifika otomasyonu

`hoodmin-acme auth|cleanup|deploy ROOT_ONLY_CONFIG.json` Debian Certbot kancalarıdır (hook). Yapılandırma root sahibi ve 0600 olmalıdır; `origin`, `domain`, `server` UUID ve süreli operatör botunun `authorization` değerini içerir. `auth` HTTP-01 yanıtını yönetim API'siyle VPN sunucusuna iletir, gerçek dış TCP80 yanıtını denetler. `cleanup` dinleyiciyi kapatır. `deploy` yalnız beklenen Certbot canlı sertifika dizinini kabul eder, PFX yükler ve genel VPN adresinde CA doğrulaması ile DER eşleşmesini denetler. HTTP yönlendirmesiyle başka hedefe API kimliği gönderilmez.

Bu işletmen tarafından yapılandırılan bir entegrasyondur; kiracının panelde kendi kendine ACME hesabı/işi açtığı AutoSign ekranı henüz yoktur. Bot en fazla 365 gün geçerlidir; süre bitmeden panelden yeni anahtar oluşturulup root yapılandırması değiştirilmelidir. Certbot iletişim e-postası, ayrıca bir hata bildirim kanalı kurulduğu anlamına gelmez; mevcut hatalar systemd günlüğündedir.

Gerçek sertifika alımında dağıtım kancası çalışır. Yenileme doğrulaması `certbot renew --dry-run` ile yapılır; deneme sırasında dağıtım kancası ayrıca zorlanmaz. Root yapılandırmalı Certbot zamanlayıcısı gerçek yenilemede PFX'i otomatik uygular.

Kaynaklar: [Certbot kullanım belgesi](https://eff-certbot.readthedocs.io/en/stable/using.html), [resmî VpnHood Linux kurulumu](https://github.com/vpnhood/VpnHood/wiki/How-to-run-VpnHood-Server-on-Linux).

## Şifreli makine dışı yedek

İsteğe bağlı `age` paketiyle `hoodmin-backup-export`, `/etc/hoodmin/backup-recipient.txt` içindeki açık alıcı anahtarına şifreler. Özel kurtarma anahtarı arşive konmaz. `latest.age` atomik değiştirilir; önceki yerel anlık görüntüler silinmez. Diğer makine yalnız şifreli dosyayı okuyan, kaynak IP ve komutla sınırlandırılmış SSH anahtarı kullanır. Örnek zamanlayıcılar paketin `examples` dizinindedir; kendiliğinden etkinleşmez.

Kurtarma anahtarını bağımsız ve güvenli bir yerde tutun; iki sunucu aynı anda kaybolursa yalnız sunucuda duran anahtar yetmez. Saklama süresi ve disk kullanımına göre temizlik ayrıca belirlenmelidir. Mevcut araçlar eski yedekleri otomatik silmez. Bu düzen, sunucuların ve yerel çalışma bilgisayarının bağımsız güvenlik denetimi yapıldığı anlamına gelmez.
