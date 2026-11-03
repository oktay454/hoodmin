# İşletim başvurusu: mevcut Hoodmin ortamı

Bu belge yalnız `oktayaktogan.com.tr` yerleşiminin adreslerini ve işletim yollarını içerir. Yeni kurulum için [genel kurulum kılavuzunu](uretim-kurulumu.md) kullanın. Parola, API anahtarı veya özel sertifika içeriği bu belgede bulunmaz.

Bu yerleşim Rust CLI, JSON yapılandırması ve doğrudan Nginx statik sunumunu kullanır. Güncel paket sürümünü aşağıdaki komutla okuyun:

```bash
ssh root@hoodmin.oktayaktogan.com.tr 'lxc-attach -n hoodmin -- dpkg-query -W hoodmin'
```

Hizmetin sistem hesabı ve PostgreSQL uygulama rolü `hoodmin` olur. Panel hesapları ve VPN anahtarları bu sistem kimliğinden ayrıdır.

## Makine ve kapsayıcılar

| Görev | SSH ana makinesi | LXC kapsayıcısı | İç adres | Dış hizmet |
|---|---|---|---|---|
| Panel | `hoodmin.oktayaktogan.com.tr` | `hoodmin` | `10.0.3.10` | HTTPS TCP443, HTTP TCP80 |
| VPN | `vh01.oktayaktogan.com.tr` | `vh01` | `10.0.3.20` | TCP443, UDP443, QUIC UDP4443; AutoSign için TCP80 |

Kapsayıcılar Debian 13 ve ayrıcalıksızdır (unprivileged); UID/GID eşlemesi 100000 tabanlıdır. Şablon adı `template-deb13` olur. VPN kapsayıcısına `/dev/net/tun` verilmiştir. Genel SSH ana makinededir; kapsayıcı SSH hizmeti dışa yönlendirilmez. NAT kuralları ana makinelerde `/etc/lxc/network-rules.conf` düzenindedir.

## Panele erişim

Adres: **https://hoodmin.oktayaktogan.com.tr/**. İlk sahip e-postası **vpn@oktayaktogan.com.tr**.

İlk parola panel kapsayıcısındaki `/root/hoodmin-first-owner.json` dosyasındadır. Kendi güvenilir terminalinizden:

```bash
ssh root@hoodmin.oktayaktogan.com.tr 'lxc-attach -n hoodmin -- cat /root/hoodmin-first-owner.json'
```

Bu komut ilk parolayı gösterir; çıktıyı paylaşmayın. Parola değiştirilmişse güncel parolayı kullanın. Sistem hesabı `hoodmin` panel giriş hesabı değildir.

Panel kapsayıcısına giriş:

```bash
ssh root@hoodmin.oktayaktogan.com.tr
lxc-attach -n hoodmin
systemctl status hoodmin.service --no-pager
```

Programlar `/usr/bin/hoodmin` ve `/usr/bin/hoodmin-cli`, ayar `/etc/hoodmin/hoodmin.json`, veriler `/var/lib/hoodmin`, veritabanı ve uygulama rolü `hoodmin` olur. Bağlantı dizesi `postgresql:///hoodmin?host=/var/run/postgresql&user=hoodmin` ile yerel eş kimlik doğrulaması (peer authentication) kullanılır.

Nginx aynı kapsayıcıda HTTPS'i sonlandırır. `/usr/share/hoodmin/web/` ağacını doğrudan sunar; `web.serve_static=false` olur. API, dinamik VPN adres belirteçleri ve sağlık yolları `127.0.0.1:3080` adresine iletilir. Arayüz `/` kökünde, sunucular `/servers`, VPN anahtarları `/access-keys` altındadır. [Sunum kılavuzu](web-sunumu.md).

## VPN ve sertifikalar

VPN sunucusu resmî hazır VpnHood 9.0.856 dağıtımıdır. Kapsayıcıdaki kurulum `/opt/VpnHoodServer`, hizmet `VpnHoodServer.service` olur. Otomatik sürüm güncellemesi kapalıdır; sürümler kontrollü doğrulamayla değiştirilir.

Panel HTTPS sertifikasını root Certbot/Nginx yönetir. VPN sertifikasını Hoodmin'deki **Sunucu → Sertifikalar → AutoSign** işi yönetir. İkisi ayrı hedeflerdir. Son geçerlilik tarihini sabit belgeye göre değil güncel sertifika/AutoSign durumundan kontrol edin.

VPN'in root Certbot yenileme kaydı `.conf.autosign-disabled` uzantısıyla devre dışıdır; ikinci yenilemeyi etkinleştirmeyin. Panelin kendi yenileme kaydı etkindir. AutoSign, Rust hizmetinin kalıcı iş kuyruğunu ve yerel CLI kancasını kullanır; eski işletmen köprüsü etkin yapılandırmada bulunmaz. Harici [bildirim kanalları](isletim-politikalari.md) desteklenir; bu yerleşimde henüz kanal tanımlanmamıştır.

## Yedekler ve bakım

- Panel kapsayıcısı: `/var/backups/hoodmin/`; şifreli aktarım dosyası `latest.age`.
- Panel `hoodmin-backup-export.timer`: Rust `hoodmin-cli backup export` komutuyla UTC 02:15, İstanbul 05:15 civarı.
- VPN ana makinesi `hoodmin-backup-pull.timer`: UTC 03:15, İstanbul 06:15 civarı. Beş dakikaya kadar rastgele gecikme vardır.
- Uzak şifreli kopya: VPN ana makinesinde `/var/backups/hoodmin-remote/`.
- Kurtarma kimliği panelde `/root/hoodmin-backup.agekey` yolundadır; bağımsız güvenli kopyası ayrıca saklanmalıdır. Sunucuyla birlikte kaybolacak tek kopyaya güvenmeyin.
- Saklama politikasını ayrıca etkinleştirin; varsayılan otomatik silme kapalıdır. [Kurtarma adımları](yedekleme.md).

`hoodmin-before-040-20261103`, panel geçişinden önceki tam kapsayıcıyı koruyan durdurulmuş ve otomatik başlamayan geri dönüş kopyasıdır. Eski paket/şema düzenini içerir. `hoodmin-before-autosign-20261019` ve `vh01-before-autosign-20261019` daha eski, yine durdurulmuş kopyalardır. Bu kopyalar güncel yedek yerine geçmez. Ana makinedeki `/root/hoodmin-deploy-040/` son aktarım yedeği ve bütünlük kanıtlarını içerir; yalnız root erişmelidir. Geri dönüş gerektiğinde güncel verileri önce ayrıca koruyun. Bakım öncesi yalnız ilgili kapsayıcıyı durdurup benzersiz adla `lxc-copy` alın. Geri dönüş kopyasını asıl kapsayıcıyla aynı IP üzerinde aynı anda çalıştırmayın. Şablonları koruyun.
