# Debian 13 üzerinde Hoodmin kurulumu

Yeni ve ayrılmış Debian 13 amd64 ortamında root olarak çalışın. Panel için `hoodmin.example.com`, VPN makinesi için ayrı bir adres kullanın. PostgreSQL 17 aynı veya uzak makinede olabilir; Hoodmin paketi PostgreSQL sunucusunu kurmaz.

## 1. DNS ve ağ

Panel alan adı panel makinesinin genel adresine gitmeli; TCP 80/443 açılmalıdır. Rust yalnız loopback üzerinde, varsayılan `127.0.0.1:3080` adresinde dinler. VPN sunucusu panelin HTTPS adresine ulaşmalıdır. VPN istemci portlarını VPN makinesine yönlendirin.

Uzak PostgreSQL için panelden veritabanı adresine TCP erişimi gerekir. Veritabanını internete sınırsız açmayın; kendi özel ağ/güvenlik duvarı politikanıza göre panelin erişimini tanımlayın.

## 2. Paketi kurma

```bash
apt update
apt install ./hoodmin_0.4.0-1~deb13u1_amd64.deb
```

Paket `hoodmin` sistem hesabını, ana anahtar dosyasını ve boş `/etc/hoodmin/hoodmin.json` yapılandırmasını hazırlar. Hizmet kendiliğinden etkinleşmez. Elle başlattığınızda henüz yapılandırılmamış olsa bile HTTP hizmeti ayakta kalır; basit bir kullanılabilirlik mesajı gösterir.

Nginx önerilen pakettir. `--no-install-recommends` kullandıysanız ayrıca kurun. PostgreSQL, PostgreSQL istemci araçları ve age zorunlu paket bağımlılığı değildir. `pg_dump` ile yedek için uygun sürümde `postgresql-client`, şifreli dışa aktarım için `age` ayrıca gerekir.

## 3. Bağlantı, veritabanı ve ilk hesap

### Bağlantı dizesi neyi gösterir?

Yerel örnek:

```text
postgresql:///hoodmin?host=/var/run/postgresql&user=hoodmin
```

`hoodmin` veritabanı adıdır. `host=/var/run/postgresql` aynı makinedeki Unix soketinin dizinidir; DNS kullanılmaz. `user=hoodmin` PostgreSQL rolüdür. Yerel eş kimlik doğrulamasında (peer authentication) sistem hesabı da `hoodmin` olur.

Uzak örnek:

```text
postgresql://hoodmin:PAROLA@db.example.com:5432/hoodmin?sslmode=verify-full&sslrootcert=/etc/hoodmin/db-ca.crt
```

`db.example.com` DNS adı, `5432` port, `/hoodmin` veritabanı, kullanıcı/parola kimlik bilgileridir. Parolanın özel karakterlerini URL kodlamasıyla yazın. TCP bağlantısında `sslmode=verify-full` zorunludur: CA ve sunucu adı doğrulanır. Özel CA kullanıyorsanız açık CA dosyasını Hoodmin hesabının okuyabildiği konuma koyun. Genel güvenilir CA için `sslrootcert` gerekmeyebilir. DNS adı sertifikadaki adla eşleşmelidir; doğrulamayı kapatmayın.

### Ayarı kaydetme

Bağlantı dizesini kendi editörünüzle `/root/hoodmin-db.url` dosyasına yazın, `chmod 0600 /root/hoodmin-db.url` uygulayın. Parolayı komut satırı argümanına veya kabuk geçmişine koymayın.

```bash
hoodmin-cli config set --database-url-file /root/hoodmin-db.url --public-origin https://hoodmin.example.com
hoodmin-cli config show
```

`config show` parolayı göstermez. Yerel rol, veritabanı adı veya uzak kullanıcı adı farklı olabilir; bağlantı dizesinde açıkça belirtin. Sistem hizmet hesabının adı `hoodmin` kalır. Ana anahtarı kurulumlar arasında rastgele değiştirmeyin.

### Veritabanını hazırlama

**Yerel seçenek:** PostgreSQL'i ayrıca kurup başlatın; bağlantı dizesinde yerel soket seçili olmalıdır:

```bash
apt install postgresql
systemctl enable --now postgresql
hoodmin-cli database init --local
```

Bu komut yerel `postgres` hesabıyla rol/veritabanı ve şemayı oluşturur. Hoodmin'in çalışması için PostgreSQL'in bu makinede olması gerekmez.

**Uzak seçenek:** PostgreSQL yöneticisinin bağlantı dizesini ayrı 0600 `/root/hoodmin-db-admin.url` dosyasına yazın. Aynı veritabanı sunucusundaki yönetim veritabanına (örneğin `/postgres`) bağlanmalıdır; TLS doğrulaması yine zorunludur.

```bash
hoodmin-cli database init --admin-url-file /root/hoodmin-db-admin.url
```

Bu komut yönetici yetkisiyle eksik rol/veritabanını oluşturur ve şema geçişlerini uygular. Yeni rolün parolası uygulama bağlantı dizesinden alınır; var olan rolün parolası kendiliğinden değiştirilmez. Şema yöneticisi, güvenlik tanımlayıcılı bakım işlevleri için SUPERUSER veya BYPASSRLS yetkisi taşımalıdır. Bu bağlantıyı yalnız kurulum/geçiş işleminde kullanın. Yönetici rolünü hizmet yapılandırmasına yazmayın. Uygulama rolü süper kullanıcı/BYPASSRLS olamaz. Veritabanı hizmeti rol oluşturmayı kısıtlıyorsa yöneticiniz rol/veritabanını önceden hazırlamalı ve şema kurulumuna uygun yönetim yetkisi sağlamalıdır.

Şema geçişleri işlem (transaction), kilit ve SHA-256 kaydıyla izlenir; komut tekrar çalıştırılabilir. İzlenmeyen mevcut şema veya değiştirilmiş geçiş dosyası reddedilir.

### İlk sahip

```bash
hoodmin-cli owner-init --email vpn@example.com
```

İlk e-posta/rastgele parola yalnız root'a özel `/root/hoodmin-first-owner.json` dosyasına yazılır. Komut çıktısına parola basılmaz. Mevcut hesap varsa işlem tekrarlanmaz. İlk girişten sonra **Hesabım** bölümünde parolayı değiştirin.

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
		proxy_set_header X-Real-IP $remote_addr;
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
hoodmin-cli service enable
hoodmin-cli service start
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

Nginx’in statikleri doğrudan sunmasını isterseniz [iki sunum biçimi ve ayrı makine kurulumu](web-sunumu.md) kılavuzunu kullanın. Tek paket `/usr/share/hoodmin/web/` ağacını kurar; ayrı Nginx’e yalnız bu ağaç kopyalanır.

## 6. İlk giriş ve VPN sunucusu

Tarayıcıdan `https://hoodmin.example.com` adresini açın. İlk sahip e-postası/parolasıyla giriş yapın; **Hesabım** bölümünden parolayı değiştirin. **Sunucular → Sunucu ekle** ile devam edin. [Elkitabı](elkitabi.md), sunucuyu bağlama ve ilk VPN anahtarını oluşturma adımlarını içerir.

## Dosyalar ve işletim

| Kaynak | Konum |
|---|---|
| Hizmet / CLI | `/usr/bin/hoodmin` / `/usr/bin/hoodmin-cli` |
| systemd hizmeti | `hoodmin.service`, `hoodmin` sistem hesabı |
| Yapılandırma | `/etc/hoodmin/hoodmin.json`, root:hoodmin 0640 |
| Ana anahtar | `/var/lib/hoodmin/master.key`, hoodmin:hoodmin 0600 |
| AutoSign dosyaları | `/var/lib/hoodmin/acme/` |
| İlk sahip bilgileri | `/root/hoodmin-first-owner.json`, 0600 |
| Statik arayüz | `/usr/share/hoodmin/web/` |
| Belgeler | `/usr/share/doc/hoodmin/docs/` |

Veritabanı adı ve rol, bağlantı dizesinden okunur. `hoodmin` sistem hesabı panel giriş hesabı değildir. Yerel soket ve uzak TLS seçenekleri aynı şema/kiracı korumasını kullanır.

Yapılandırma veya veritabanı erişilemiyorsa hizmet kapanmaz. Tarayıcı kısa bir hata gösterir; ayrıntı `journalctl -u hoodmin.service` içindedir. Bağlantı geri geldiğinde otomatik toparlanır. `/healthz` HTTP sürecinin çalıştığını, `/readyz` veritabanı/uygulamanın hazır olduğunu bildirir. Dinleme adresi değişikliği için hizmeti yeniden başlatın.

## Yükseltme

Bu sürüm yeni JSON yapılandırması ve Rust CLI düzeniyle temiz kurulum içindir; önceki geliştirme paketlerinden yerinde geçiş yapılmaz. Aynı yapılandırma/şema düzenini kullanan sonraki sürümlerde önce [yedek alın](yedekleme.md), ardından bakım penceresinde:

```bash
hoodmin-cli service stop
apt install ./hoodmin_YENI_SURUM_amd64.deb
hoodmin-cli database init --admin-url-file /root/hoodmin-db-admin.url
hoodmin-cli service start
```

Yerel kurulumda yönetici dosyası yerine `--local` kullanılır. Ana anahtar ve yapılandırmayı koruyun. Geri dönüşte veritabanı, anahtar ve paket sürümü birlikte değerlendirilir.

## Kaynaktan derleme

Debian `build-essential`, `debhelper`, `dpkg-dev`, `pkg-config`, `libssl-dev`, Python 3 ve `rust-toolchain.toml` içindeki Rust/Cargo zinciri gerekir. Python yalnız derleme lisans raporunda kullanılır; kurulu hizmet/CLI Rust'tır.

```bash
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
dpkg-buildpackage -b -us -uc
```

Rust yalnız rustup ile kuruluysa dpkg paket kaydını göremez; bütün bağımlılıkları denetledikten sonra `dpkg-buildpackage -d -b -us -uc` kullanılabilir. [Geliştirme başvurusu](dogrulama.md).
