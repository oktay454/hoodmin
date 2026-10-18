# Yeni sunucu ortamı — 18 Ekim 2026

Bu kayıt, üretimde kullanılmak üzere hazırlanan yeni ortamın mevcut durumudur. Uzun süreli yük, bağımsız güvenlik denetimi veya kapalı kaynak Console ile tam eşdeğerlik tamamlanmış sayılmaz.

| İşlev | SSH ana makinesi | Kapsayıcı | İç adres | Dış erişim |
|---|---|---|---|---|
| Hoodmin | hoodmin.oktayaktogan.com.tr | hoodmin | 10.0.3.10 | TCP80/443; DNS A 93.115.79.233, sağlayıcı NAT arkasında |
| VPN | vh01.oktayaktogan.com.tr | vh01 | 10.0.3.20 | TCP443, UDP443, QUIC UDP4443; DNS A 213.142.151.232 |

Her ikisi Debian 13.7 şablonunun kopyasıdır. Şablonlar korunmuştur. Ana makine temeli kullanıcının [install-container-environment betiği](https://raw.githubusercontent.com/oktay454/Betikler/refs/heads/main/install-container-environment) ile hazırlanmıştı; betik incelendi, yeniden çalıştırılmadı. Kapsayıcılar ayrıcalıksızdır (unprivileged), UID/GID 0–65535 ana makinede 100000–165535 aralığına eşlenir. İç içe kapsayıcı izni kapalı, ana makinenin tüm sertifika dizini bağlanmamıştır. VPN'e yalnız gerekli `/dev/net/tun` aygıtı verilmiştir. Dış SSH ana makinede kalır; kapsayıcı SSH portları dışarı yönlendirilmez. Kapsayıcı otomatik başlangıcı etkindir.

Ağ yönlendirmeleri kullanıcının `/etc/lxc/network-rules.conf` düzenine eklendi; önceki dosyalar ana makinelerde `*-network-rules.before` olarak saklandı. Panel 80/443 dış erişimi ve yeniden kapsayıcı başlatma sonrası erişim doğrulandı. Tam ana makine güç kesintisi sınanmadı.

## Panel ve ilk hesap

- Adres: https://hoodmin.oktayaktogan.com.tr/
- Paket: `hoodmin 0.1.0-3~deb13u1`, Debian 13 amd64.
- Hizmet: `hoodmin.service`; ikili `/usr/bin/hoodmin`; veri `/var/lib/hoodmin`; ayar `/etc/hoodmin/hoodmin.env`.
- PostgreSQL yerel eş kimlik doğrulaması (peer); uygulama rolü `vhmgr`, veritabanı `vpnhood_manager`. Uygulama rolü süper kullanıcı değildir ve RLS atlayamaz.
- İlk sahip: `vpn@oktayaktogan.com.tr`. Rastgele parola yalnız kapsayıcıdaki `/root/hoodmin-first-owner.json` içinde 0600 izinleriyle tutulur. Burada gerçek parola veya API anahtarı yoktur. Yerel dosyaya ilk parola kopyalanmadı.
- Ana makineden erişim: `lxc-attach -n hoodmin -- cat /root/hoodmin-first-owner.json`. Bu komut parolayı gösterir; yalnız kendi güvenilir SSH oturumunuzda kullanın. İlk girişten sonra hesap ekranından değiştirin.
- Sınama ortamının veritabanı/hesapları üretime taşınmadı. Eski `vhm-test` ortamı ayrıca korunur.

Nginx aynı kapsayıcıdaki loopback Rust hizmetine bağlanır; istemciden gelen vekil IP başlığını değiştirilmeden kabul etmez. Tarayıcı giriş sayfası geçerli TLS ile açıldı. Parola yerel bilgisayara aktarılmadığından üretim sahibinin tarayıcı oturumuyla görsel inceleme yapılmadı; işlevler API ve kapsayıcı içindeki sahip oturumuyla doğrulandı.

## VPN ve sertifikalar

Resmî hazır VpnHood **9.0.856** `/opt/VpnHoodServer` altında kuruldu; sunucu kaynak kodu derlenmedi. `VpnHoodServer.service` başlangıçta etkin. Uygulamanın otomatik sürüm güncellemesi kapalıdır; uyumluluk sınamasıyla kontrollü güncelleme yapılmalıdır. TUN arayüzü, NAT ve UDP/TCP/QUIC dinleyicileri doğrulandı.

Panel ve VPN alan adları gerçek Let’s Encrypt sertifikası kullanır; mevcut sertifikalar **16 Ocak 2027** tarihine kadar geçerlidir. VPN sertifikası panel kapsayıcısındaki Debian Certbot ile alınır:

1. Root sahibi 0600 `/etc/hoodmin/acme/vh01.json`, yalnız bu kiracının operatör botu kimliğini içerir.
2. HTTP-01 kancası Hoodmin API'sinden VPN'e doğrulamayı aktarır ve dış TCP80 yanıtını kontrol eder.
3. Temizleme kancası geçici HTTP80 dinleyicisini kapatır.
4. Gerçek sertifika dağıtım kancası PFX'i Hoodmin'e yükler; genel VPN adresinde CA doğrulaması ve DER eşleşmesi yapar.
5. Debian `certbot.timer` yenilemeleri yürütür. `certbot renew --dry-run` başarılıdır; bu sınamada dağıtım kancası ayrıca zorlanmamıştır.

`ACME vh01` botu operatör yetkisiyle 365 gün geçerlidir. Süre bitmeden panelden yeni anahtar üretilip root yapılandırması yenilenmelidir. Hata için harici e-posta/izleme kanalı henüz yoktur; systemd/Certbot günlükleri izlenmelidir. Sertifika iletişim e-postası otomatik alarm hizmeti değildir. İşletmen yapılandırması hazırdır; çok kiracılı panelde kendi kendine ACME işi oluşturma ekranı henüz yoktur.

## Yedekleme ve geri yükleme

Panelde `hoodmin-backup-export.timer` her gün İstanbul saati yaklaşık **05:15**, VPN ana makinesinde `hoodmin-backup-pull.timer` yaklaşık **06:15** çalışır; en fazla beş dakikalık rastgele gecikme vardır. Anlık görüntü, PostgreSQL dökümü, ana anahtar, Hoodmin/Certbot ayarları, sertifikalar ve paket sürümünü içerir.

- Yerel anlık görüntüler: panel kapsayıcısında `/var/backups/hoodmin/`.
- Şifreli aktarım: `age` ile kimliği doğrulanmış şifreleme; `latest.age` atomik yenilenir.
- Ayrı makine kopyası: VPN ana makinesinde `/var/backups/hoodmin-remote/`.
- Okuma anahtarı yalnız VPN ana makinesinin genel IP'sinden, yalnız sabit şifreli dosyayı okuyan SSH komutuna izin verir; kabuk, TTY ve bağlantı yönlendirme yoktur. SSH sunucu anahtarı sabittir.
- Kurtarma kimliği panelde `/root/hoodmin-backup.agekey`; ayrıca yerel proje `work/production/hoodmin-backup.agekey` içinde 0600 kopyası vardır. Kimliği bağımsız güvenli saklama alanına ayrıca alın; Git'e koymayın.
- Yedekler kendiliğinden silinmez. Saklama süresi ve disk kullanım izlemesi ayrıca belirlenmelidir.

Şifreli dosyanın iki makinedeki SHA-256 özeti eşleşti. Age çözme ve iç özetler geçti. Ayrı `hoodmin_restore_validation_20261018` veritabanına yükleme, 2 hesap / 1 sunucu / 8 geçiş kaydı kontrolü ve yedeklenen ana anahtarla sunucu yapılandırmasını çözerek anahtar üretme geçti. Bu yalnız ayrı kurtarma veritabanında yapıldı; canlı veritabanının üzerine yazılmadı. Geçici kurtarma veritabanı sonra kaldırıldı.

## Doğrulamalar

- 22 Rust testi, Clippy ve Debian ikili paket derlemesi geçti. Lintian paket hata/uyarısı yoktur; root ile çalıştırılmasına ilişkin genel araç uyarısı ayrı tutulur.
- Yeni paket kuruldu, şema geçişi tekrar çalıştırılıp özetler doğrulandı, paket yükseltmesi sınandı. Ana anahtar baytları değişmedi.
- Sertifika değişiminde DNS çözümleme bayrağının korunması düzeltildi; iki gerileme sınaması ve grup PFX sınaması geçti.
- Gerçek istemci TCP443 bağlantısı kurdu. Gerçek UDP DNS verisi TCP, UDP ve QUIC paket kanallarından geçti; istenen protokole geri düşme olmadığı denetlendi. Sunucunun TUN/NAT sayaçları arttı. Bu, masaüstü işletim sisteminin bütün rotalarının sınandığı anlamına gelmez.
- Kapsayıcı yeniden başlatmalarından sonra panel HTTPS'i, hizmetlerin otomatik başlangıcı, agent yapılandırma onayı ve QUIC üzerinden UDP veri geçişi tekrar doğrulandı.
- Tema/dil HTTPS sınaması yeni genel alan adında geçti. Eski ayrılmış sınama sunucusunda PFX/HTTP01/TCP hız/QUIC/ağ filtresi dizisi tekrar geçti; özel ağ izni ve HTTP01 kapatıldı.
- Kısa ömürlü dağıtım sınama anahtarı doğrulama sonunda iptal edilir; gerçek kullanıcı erişim anahtarı olarak paylaşılmaz.

## Kurulan başlıca paketler

Panelde Debian depolarından Nginx 1.26.3, PostgreSQL 17.11, Certbot 4.0.0, age 1.2.1; Python 3, OpenSSH istemcisi, OpenSSL ve sertifika bağımlılıkları. VPN'de Debian'dan `libicu76 76.1`, `libnuma1 2.0.19`, `iptables 1.8.11`, OpenSSL ve temel indirme araçları. Resmî hazır VpnHood kurucusu Microsoft kaynağından `libmsquic 2.6.3` kurdu; geçici Microsoft depo paketi kurucu tarafından kaldırıldı. MSQuic güncellemesi resmî kurucu/kontrollü bakım sürecinde izlenmelidir.

## Ayrı onay bekleyen UID/GID taşıması

Mevcut eşleme 100000 tabanlıdır ve ana makinedeki ilkbyte/debian kullanıcılarının alt kimlik aralığıyla ortaktır. Kullanıcı yetkilerinden ayrı bir eşleme için **200000–265535** hedef aralığı önerildi. Salt okunur ön inceleme: Hoodmin 29.731, VPN 23.575 tekil dosya nesnesi; beklenmeyen sahiplik ve dosya yeteneği yok, hedef aralıkta çakışma yok. Sayılar sonradan günlüklerle değişebilir.

Bu taşıma uygulanmadı: otomatik onay denetimi kapsayıcıların durdurulması, tüm dosya sahipliklerinin ve ana makine alt kimlik kayıtlarının değiştirilmesi nedeniyle adımı reddetti. Mevcut çalışan düzen korunur. Kullanıcı onaylarsa yalnız `hoodmin` ve `vh01` durdurulur; önce tam durdurulmuş dosya ağacı/yapılandırma kopyası alınır, sahiplik ve eşleme taşınır, izin bitleri korunur, hizmet/TLS/istemci kontrolleri tekrarlanır. Şablonlar değiştirilmez. Hata halinde kopyadan eski dosya ağacı ve eşleme geri konur. Ön inceleme aracı `scripts/plan-lxc-idmap.py` hiçbir dosyayı değiştirmez.
