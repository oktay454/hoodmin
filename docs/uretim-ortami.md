# İşletim başvurusu: mevcut Hoodmin ortamı

Bu belge yalnız `oktayaktogan.com.tr` yerleşiminin adreslerini ve işletim yollarını içerir. Yeni kurulum için [genel kurulum kılavuzunu](uretim-kurulumu.md) kullanın. Parola, API anahtarı veya özel sertifika içeriği bu belgede bulunmaz.

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

Bu komut ilk parolayı gösterir; çıktıyı paylaşmayın. Parola değiştirilmişse güncel parolayı kullanın. Sistem hesabı `vhmgr` panel giriş hesabı değildir.

Panel kapsayıcısına giriş:

```bash
ssh root@hoodmin.oktayaktogan.com.tr
lxc-attach -n hoodmin
systemctl status hoodmin.service --no-pager
```

Program `/usr/bin/hoodmin`, ayar `/etc/hoodmin/hoodmin.env`, veriler `/var/lib/hoodmin`, veritabanı `vpnhood_manager`, hizmet/DB rolü `vhmgr` olur. Nginx aynı kapsayıcıda HTTPS'i sonlandırıp `127.0.0.1:3080` adresine iletir.

## VPN ve sertifikalar

VPN sunucusu resmî hazır VpnHood 9.0.856 dağıtımıdır. Kapsayıcıdaki kurulum `/opt/VpnHoodServer`, hizmet `VpnHoodServer.service` olur. Otomatik sürüm güncellemesi kapalıdır; sürümler kontrollü doğrulamayla değiştirilir.

Panel HTTPS sertifikasını root Certbot/Nginx yönetir. VPN sertifikasını Hoodmin'deki **Sunucu → Sertifikalar → AutoSign** işi yönetir. İkisi ayrı hedeflerdir. Son geçerlilik tarihini sabit belgeye göre değil güncel sertifika/AutoSign durumundan kontrol edin.

VPN'in root Certbot yenileme kaydı `.conf.autosign-disabled` uzantısıyla devre dışıdır; ikinci yenilemeyi etkinleştirmeyin. Panelin kendi yenileme kaydı etkindir. İşletmen köprüsüne ait `/etc/hoodmin/acme/vh01.json` özel bot bilgisi içerir; geçerli AutoSign akışı bunu kullanmaz. Harici hata bildirimi hizmeti yoktur.

## Yedekler ve bakım

- Panel kapsayıcısı: `/var/backups/hoodmin/`; şifreli aktarım dosyası `latest.age`.
- Panel `hoodmin-backup-export.timer`: UTC 02:15, İstanbul 05:15 civarı.
- VPN ana makinesi `hoodmin-backup-pull.timer`: UTC 03:15, İstanbul 06:15 civarı. Beş dakikaya kadar rastgele gecikme vardır.
- Uzak şifreli kopya: VPN ana makinesinde `/var/backups/hoodmin-remote/`.
- Kurtarma kimliği panelde `/root/hoodmin-backup.agekey` yolundadır; bağımsız güvenli kopyası ayrıca saklanmalıdır. Sunucuyla birlikte kaybolacak tek kopyaya güvenmeyin.
- Eski yedekler otomatik silinmez; disk/saklama politikasını ayrıca yönetin. [Kurtarma adımları](yedekleme.md).

`hoodmin-before-autosign-20261019` ve `vh01-before-autosign-20261019` durdurulmuş, otomatik başlamayan geri dönüş kopyalarıdır. Eski sürüm içerirler; güncel yedek yerine geçmezler. Bakım öncesi yalnız ilgili kapsayıcıyı durdurup benzersiz adla `lxc-copy` alın. Geri dönüş kopyasını asıl kapsayıcıyla aynı IP üzerinde aynı anda çalıştırmayın. Şablonları koruyun.
