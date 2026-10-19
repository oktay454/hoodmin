# Debian 13 üzerinde Hoodmin kurulumu

Bu kılavuz yeni ve ayrılmış Debian 13 amd64 kurulumunu anlatır. Komutları panel makinesinde root olarak çalıştırın. `hoodmin.example.com` ve `vpn@example.com` değerlerini kendi alan adınız/e-postanızla değiştirin. VPN sunucusu ayrı kurulur; onun için [elkitabını](elkitabi.md#ilk-vpn-sunucusunu-bağlama) izleyin.

## 1. DNS ve ağ hazırlığı

- Panel alan adının A/AAAA kayıtları panelin genel adresine gitmelidir. Kullanmadığınız/ulaşılamayan IPv6 kaydı bırakmayın.
- Panel için dış TCP 80 ve 443'ü açın. NAT varsa kapsayıcıdaki aynı portlara yönlendirin.
- Rust yalnız `127.0.0.1:3080` üzerinde dinler; bu portu dışarı açmayın. PostgreSQL yerel UNIX soketi kullanır.
- VPN sunucusu panelin HTTPS adresine ulaşabilmelidir. VPN istemcilerinin portları VPN sunucusuna yönlendirilir.

## 2. Paketi ve veritabanını kurma

İndirdiğiniz `.deb` dosyasının bulunduğu dizinde:

```bash
apt update
apt install ./hoodmin_0.1.0-8~deb13u1_amd64.deb
systemctl enable --now postgresql
hoodmin-db-setup
```

APT çalışma bağımlılıklarını kurar; Nginx önerilen pakettir, `--no-install-recommends` kullandıysanız ayrıca kurun. Panel AutoSign için Certbot paket bağımlılığıdır; panelin Nginx eklentisine ihtiyacı yoktur.

`hoodmin-db-setup`, veritabanını ve şemayı kurar; tekrar çalıştırılabilir. Uygulanmış geçişlerin SHA-256 özetini denetler. İzlenmeyen mevcut şema veya özet uyuşmazlığında devam etmeyin; geçiş dosyalarını değiştirmeyin. Paket hizmeti kendiliğinden etkinleştirmez.

## 3. İlk sahip hesabını oluşturma

```bash
hoodmin-create-owner vpn@example.com https://hoodmin.example.com
```

Bu komut çalışma alanını ve ilk sahip hesabını oluşturur, `VHM_PUBLIC_ORIGIN` ayarını yazar. HTTPS adresine yol veya sonda `/` eklemeyin; standart dışı port varsa adrese dahil edin. Başlangıç parolası `/root/hoodmin-first-owner.json` dosyasındadır; 0600 izinlidir, komut çıktısına yazılmaz. Dosyayı yalnız kendi güvenilir root terminalinizde okuyun. Hesap zaten varsa komut yeniden hesap açmaz veya parolayı sıfırlamaz.

## 4. Panelin HTTPS sertifikasını alma

Aynı makinedeki Nginx için önce yalnız HTTP bloğu hazırlayın:

```nginx
server {
	listen 80;
	server_name hoodmin.example.com;
	location ^~ /.well-known/acme-challenge/ {
		root /var/www/html;
		try_files $uri =404;
	}
	location / {
		return 404;
	}
}
```

Bunu `/etc/nginx/sites-available/hoodmin` dosyasına kaydedin. Aynı alan adı için başka etkin blok bırakmayın. Ardından:

```bash
install -d -m 0755 /var/www/html/.well-known/acme-challenge
ln -s /etc/nginx/sites-available/hoodmin /etc/nginx/sites-enabled/hoodmin
nginx -t
systemctl reload nginx
certbot certonly --webroot -w /var/www/html --domain hoodmin.example.com --email vpn@example.com --agree-tos --non-interactive
```

`--agree-tos`, sertifika otoritesinin hizmet koşullarını kabul eder; komutu çalıştırmadan önce koşulları değerlendirin. Geçerli sertifikanız zaten varsa bu adımı atlayıp sonraki blokta kendi sertifika yollarınızı kullanın.

## 5. Nginx ters vekilini etkinleştirme

Aynı dosyayı aşağıdaki iki blokla değiştirin:

```nginx
server {
	listen 80;
	server_name hoodmin.example.com;
	location ^~ /.well-known/acme-challenge/ {
		root /var/www/html;
		try_files $uri =404;
	}
	location / {
		return 301 https://hoodmin.example.com$request_uri;
	}
}
server {
	listen 443 ssl;
	server_name hoodmin.example.com;
	ssl_certificate /etc/letsencrypt/live/hoodmin.example.com/fullchain.pem;
	ssl_certificate_key /etc/letsencrypt/live/hoodmin.example.com/privkey.pem;
	ssl_protocols TLSv1.2 TLSv1.3;
	server_tokens off;
	client_max_body_size 1m;
	location / {
		proxy_pass http://127.0.0.1:3080;
		proxy_http_version 1.1;
		proxy_set_header Host $host;
		proxy_set_header X-Forwarded-Proto https;
		proxy_set_header X-Forwarded-For $remote_addr;
		proxy_set_header Connection "";
		proxy_buffering off;
		proxy_read_timeout 3600s;
	}
}
```

Yapılandırmayı kontrol edip paneli başlatın:

```bash
nginx -t
systemctl reload nginx
systemctl enable --now hoodmin.service
curl --fail --show-error https://hoodmin.example.com/healthz
```

Sertifika yenilenince Nginx'i yeniden yüklemek için `/etc/letsencrypt/renewal-hooks/deploy/hoodmin-nginx` dosyasına şunu yazın ve `chmod 0755` uygulayın:

```sh
#!/bin/sh
set -e
/usr/sbin/nginx -t
/bin/systemctl reload nginx
```

```bash
systemctl enable --now certbot.timer
certbot renew --dry-run
```

Bu Certbot zamanlayıcısı **panelin HTTPS sertifikasını** yönetir. VPN sertifikası için panelden [AutoSign](autosign.md) kullanın; aynı VPN sertifikasını iki farklı zamanlayıcıya bağlamayın. Ayrı VPS üzerinden vekilleme gerekiyorsa [çok katmanlı vekil başvurusunu](vps-ters-vekil.md) okuyun.

## 6. İlk giriş ve VPN sunucusu

Tarayıcıdan `https://hoodmin.example.com` adresini açın. İlk sahip e-postası/parolasıyla giriş yapın; **Hesabım** bölümünden parolayı değiştirin. **Sunucular → Sunucu ekle** ile devam edin. [Elkitabı](elkitabi.md), sunucuyu bağlama ve ilk VPN anahtarını oluşturma adımlarını içerir.

## Dosyalar ve teknik adlar

| Kaynak | Değer |
|---|---|
| Program / systemd hizmeti | `/usr/bin/hoodmin` / `hoodmin.service` |
| Ayarlar | `/etc/hoodmin/hoodmin.env` |
| Veriler / ana anahtar | `/var/lib/hoodmin` / `/var/lib/hoodmin/master.key` |
| AutoSign hesapları ve özel anahtarları | `/var/lib/hoodmin/acme/` |
| İlk sahip bilgileri | `/root/hoodmin-first-owner.json` |
| Sistem kullanıcısı / PostgreSQL rolü | `hoodmin` / `hoodmin` |
| Veritabanı | `vpnhood_manager` |
| Geçici SSH çalışma alanı | `/run/vpnhood-manager` |
| Yüklü belgeler | `/usr/share/doc/hoodmin/` |

`hoodmin` ayrıcalıksız hizmet hesabıdır; **panel giriş kullanıcı adı değildir**. PostgreSQL eş kimlik doğrulaması (peer authentication) sistem kullanıcısı ile aynı adlı rolü eşleştirir. Dosya sahiplikleri ve SQL izinleri de bu role bağlıdır. Rol süper kullanıcı değildir ve RLS atlayamaz.

Ortam ayarları `DATABASE_URL`, `VHM_MASTER_KEY_FILE`, `VHM_PUBLIC_ORIGIN`; çerez adları `__Host-vhm_*` biçimindedir. Cargo derlemesi `target/release/vpnhood-manager` üretir; Debian paketi bunu `/usr/bin/hoodmin` olarak kurar.

## Yükseltme

Bu sürüm temiz kurulum içindir. `0.1.0-8~deb13u1` öncesindeki hesap/şema düzeninden yerinde yükseltme yapılmaz; paket işlemi değişiklik yapmadan reddedilir. Mevcut kurulum ve yedeği korunarak ayrı, boş bir ortam kullanılmalıdır. Aşağıdaki adımlar aynı hesap/şema düzenini kullanan sonraki sürümler içindir.

Yeni paketin sürümünü/destek hedefini kontrol edin. Önce [yedek alın ve ayrı hedefte kurtarmayı doğrulayın](yedekleme.md). Bakım penceresinde:

```bash
hoodmin-backup
systemctl stop hoodmin.service
apt install ./hoodmin_YENI_SURUM_amd64.deb
hoodmin-db-setup
systemctl start hoodmin.service
systemctl status hoodmin.service --no-pager
curl --fail --show-error https://hoodmin.example.com/healthz
```

Mevcut `master.key` ve `hoodmin.env` dosyalarını koruyun. Şema güncellenmeden hizmeti başlatmayın. Hata durumunda günlükleri inceleyin; yeni şema üstünde yalnız eski ikiliyi kurmak güvenli geri dönüş sayılmaz. Veritabanı, anahtar ve ilgili paket sürümünü birlikte kurtarın. Eski QA `vpnhood-manager.service` kurulumu paket tarafından otomatik taşınmaz.

## Kaynaktan paket oluşturma

Derleme için Debian `build-essential`, `debhelper`, `dpkg-dev`, `pkg-config`, `libssl-dev`, Python 3 ve `rust-toolchain.toml` içindeki Rust/Cargo zinciri gerekir. Çalışma bağımlılıkları `debian/control` dosyasındadır.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
dpkg-buildpackage -b -us -uc
```

`dpkg-buildpackage` Rust sınamalarını çalıştırır ve dinamik kitaplık bağımlılıklarını çıkarır. Rust/Cargo yalnız rustup ile kuruluysa dpkg bunları paket olarak göremez; sürümleri ve bütün diğer derleme bağımlılıklarını denetledikten sonra `-d` kullanılabilir. `work/` ve özel anahtarlar pakete girmez. [Geliştirme başvurusu](dogrulama.md).
