# VPS üzerinden sınama paneli

Tarayıcı → `https://vhm-test.oktayaktogan.com.tr` → VPS Nginx (`43.229.92.120`) → `https://46.196.34.83:40443` → kapsayıcı Nginx → Rust (`127.0.0.1:3080`).

## VPS'e uygulanacaklar

1. Alan adının A kaydı `43.229.92.120` olmalı. AAAA kaydı varsa aynı hizmeti sunan VPS IPv6 adresine işaret etmeli.
2. Kullanıcının verdiği `fullchain.pem` / `privkey.pem` yollarındaki sertifika `vhm-test.oktayaktogan.com.tr` adını (veya `*.oktayaktogan.com.tr` joker adını) kapsamalı.
3. `vhm-test-ca.crt` açık sertifikasını `/etc/nginx/vhm-test-ca.crt`, yapılandırmayı `/etc/nginx/conf.d/vhm-test.conf` olarak yerleştir. Özel CA anahtarı paylaşılmaz; CA yalnız bu vekil bağlantısında kullanılır, sistemin genel güven deposuna eklenmez.
4. Aynı alan adı için önceden yüklenen sunucu bloğu varsa onun yerine uygula; yinelenen bir blok bırakma.
5. VPS'ten `46.196.34.83:40443` çıkışı ve VPS'e 80/443 erişimi açık olmalı.

İndirilen iki dosyanın bulunduğu dizinde, VPS üzerinde root olarak:

```bash
install -o root -g root -m 0644 vhm-test-ca.crt /etc/nginx/vhm-test-ca.crt
install -o root -g root -m 0644 vhm-test.conf /etc/nginx/conf.d/vhm-test.conf
nginx -t
# Yalnız önceki sınama başarılıysa:
systemctl reload nginx
```

Bu yapılandırma sertifika üretmez. Let's Encrypt sertifikasını kullanıcı yönetecek. Mevcut sertifika yenileme yöntemi HTTP-01 kullanıyorsa onun `/.well-known/acme-challenge/` kuralını 80 sunucu bloğuna uyarlamak gerekir; bu dosyanın 80 bloğu tüm istekleri HTTPS'e yönlendirir.

## Kapsayıcı tarafı ve geçiş sırası

- Yalnız `43.229.92.120/32` güvenilir vekil (trusted proxy) olarak kabul edilir; Nginx `X-Real-IP` başlığını yalnız bu kaynaktan kullanır. VPS bu başlığı ziyaretçinin verdiği değerden bağımsız yeniden yazar.
- VPS yapılandırması hazır olduktan sonra kapsayıcıdaki `VHM_PUBLIC_ORIGIN` değeri `https://vhm-test.oktayaktogan.com.tr` yapılır ve Rust hizmeti yeniden başlatılır. Bu ayar giriş ve form işlemlerindeki kaynak (Origin) doğrulaması için gereklidir.
- Eski `3443` adresindeki panel yönlendirmesi yeni alan adına geçirilir. Mevcut `/api/agent/` yolu korunur; bağlı sınama VPN sunucularının iletişimi devam eder.
- Alan adı değiştiğinde tarayıcıda yeniden oturum açılır; çerez başka bir alan adına taşınmaz.
- Geçişten sonra eski IP ile panel formu gönderilmesi reddedilir; erişim yeni alan adından yapılır.

## Sertifika ömrü

VPS'teki Let's Encrypt sertifikasından ayrı olarak arka uçta bir sınama CA'sı vardır. Dağıtılan CA **14 Kasım 2026, 21:13:13 UTC** tarihinde biter; mevcut sunucu sertifikası 15 Kasım'da biter. Bu tarihten önce arka uç sertifikaları yenilenmeli ve VPS'e yeni açık CA sertifikası aktarılmalıdır. Bu yerleşim sınama içindir.

## Hazırlık doğrulaması — 16 Ekim 2026

- Kapsayıcıya VPS'in kesin IP adresi için güven eklendi; Nginx yapılandırma sınaması başarılı.
- Dağıtılan açık CA kopyası kaynak sertifikayla SHA-256 üzerinden eşleşiyor; arka uç sertifikasının `yapayzeka` adı doğrulandı.
- VPS sunucu bloğu geçici loopback portları, mevcut sınama sertifikaları ve yerel arka uç adresiyle Nginx 1.26.3 üzerinde sınandı. TLS doğrulaması açıkken ters vekilden `/healthz` başarılı; HTTP yönlendirmesi yolu ve sorguyu koruyor.
- VPS devreye alındı. Genel DNS ve güvenilir HTTPS bağlantısı doğrulandı; `VHM_PUBLIC_ORIGIN=https://vhm-test.oktayaktogan.com.tr` uygulandı. Eski panel yönlendirmesi yeni alan adına geçirildi; `/api/agent/` eski yerel adresinde korunuyor.

## Devreye alma doğrulaması

VPS'te yapılandırma yüklendikten sonra:

```bash
curl --fail --show-error https://vhm-test.oktayaktogan.com.tr/healthz
```

Ardından tarayıcıda `/panel` üzerinden giriş, form kaynak doğrulaması ve canlı olay akışı sınanır. VPS'in gerçek sertifikası ve DNS kaydı bu aşamada doğrulanır.

Yönergeler: [Nginx vekil modülü](https://nginx.org/en/docs/http/ngx_http_proxy_module.html).

## Tamamlanan geçiş sınaması — 16 Ekim 2026

- Uygulama içi tarayıcıda sertifika uyarısı olmadan sınama hesabıyla oturum açıldı.
- Yeni kaynak (Origin) ile giriş başarılı; eski IP kaynağı 403 ile reddediliyor. Oturum çerezinin `__Host`, `Secure`, `HttpOnly`, `SameSite=Strict` ve kök yol özellikleri doğrulandı.
- VPS üzerinden iki ardışık canlı sayaç (SSE) bildirimi alındı. HTTP sınama oturumu kapatıldı; kullanıcıya gösterilen tarayıcı oturumu açık bırakıldı.
- Sunucular, erişim anahtarları, VPN oturumları, denetim kaydı ve panel kullanıcıları tarayıcıda gezildi. Sunucu formu 300 piksel genişliğinde; ad kutusu formun içinde ve etiketi üstünde. Masaüstünde sayfa yatay taşmıyor.
- Hizmetin canlı bağlantılar nedeniyle kapanışta uzun beklemesini sınırlamak için dağıtım ek ayarına (drop-in) `TimeoutStopSec=15s` eklendi.
- Mevcut “Sınama B” kaydı çevrimdışı; canlı sayaç akışının çalışması VPN sunucusunun çevrimiçi olduğu anlamına gelmez.
