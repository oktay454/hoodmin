# Debian 13 üzerinde Hoodmin kurulumu

Yeni ve ayrılmış Debian 13 amd64 ortamında root olarak çalışın. Panel için `hoodmin.example.com`, VPN makinesi için ayrı bir adres kullanın. PostgreSQL 17 aynı veya uzak makinede olabilir; Hoodmin paketi PostgreSQL sunucusunu kurmaz.

## 1. DNS ve ağ

Panel alan adı panel makinesinin genel adresine gitmeli; TCP 80/443 açılmalıdır. Rust yalnız loopback üzerinde, varsayılan `127.0.0.1:3080` adresinde dinler. VPN sunucusu panelin HTTPS adresine ulaşmalıdır. VPN istemci portlarını VPN makinesine yönlendirin.

Uzak PostgreSQL için panelden veritabanı adresine TCP erişimi gerekir. Veritabanını internete sınırsız açmayın; kendi özel ağ/güvenlik duvarı politikanıza göre panelin erişimini tanımlayın.

## 2. Paketi kurma

```bash
apt update
apt install ./hoodmin_0.5.0-2~deb13u1_amd64.deb
```

Paket `hoodmin` sistem hesabını, ana anahtar dosyasını ve boş `/etc/hoodmin/hoodmin.json` yapılandırmasını hazırlar. Hizmet kendiliğinden etkinleşmez. Elle başlattığınızda henüz yapılandırılmamış olsa bile HTTP hizmeti ayakta kalır; basit bir kullanılabilirlik mesajı gösterir.

Nginx önerilen pakettir. `--no-install-recommends` kullandıysanız ayrıca kurun. PostgreSQL, PostgreSQL istemci araçları ve age zorunlu paket bağımlılığı değildir. `pg_dump` ile elle veya otomatik yükseltme öncesi yedek için uygun sürümde `postgresql-client`, şifreli dışa aktarım için `age` ayrıca gerekir.

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

Paketle gelen Nginx örneğini etkin yapılandırmaya kopyalayın:

```bash
install -m 0644 /usr/share/doc/hoodmin/examples/nginx-hoodmin.conf /etc/nginx/sites-available/hoodmin
```

Dosyadaki `hoodmin.example.com` adını ve sertifika yollarını kendi kurulumunuza göre düzenleyin. Örnek, `sites-enabled` bağlantısıyla etkinleştirilir. Bu bağlantıyı sertifika alma adımında oluşturduysanız aynı bağlantı kullanılır. Kaynak deposundaki karşılığı `deploy/nginx-hoodmin.conf` dosyasıdır.

Yapılandırmayı kontrol edip paneli başlatın:

```bash
ln -sfn /etc/nginx/sites-available/hoodmin /etc/nginx/sites-enabled/hoodmin
nginx -t
systemctl reload nginx
hoodmin-cli service enable
hoodmin-cli service start
curl --fail --show-error --retry 5 --retry-delay 1 https://hoodmin.example.com/readyz
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

Bu Certbot zamanlayıcısı **panelin HTTPS sertifikasını** yönetir. VPN sertifikası için panelden [AutoSign](autosign.md) kullanın; aynı VPN sertifikasını iki farklı zamanlayıcıya bağlamayın. Ayrı VPS üzerinden vekilleme gerekiyorsa [çok katmanlı vekil başvurusunu](web-sunumu.md#4-ayrı-vps-bütün-istekleri-vekiller) okuyun.

Nginx’in statikleri doğrudan sunmasını isterseniz [iki sunum biçimi ve ayrı makine kurulumu](web-sunumu.md) kılavuzunu kullanın. Tek paket `/usr/share/hoodmin/web/` ağacını kurar; ayrı Nginx’e yalnız bu ağaç kopyalanır.

## 6. İlk giriş ve VPN sunucusu

Tarayıcıdan `https://hoodmin.example.com` adresini açın. İlk sahip e-postası/parolasıyla giriş yapın; **Hesabım** bölümünden parolayı değiştirin. **Sunucular → Sunucu ekle** ile devam edin. [Elkitabı](elkitabi.md), sunucuyu bağlama ve ilk VPN anahtarını oluşturma adımlarını içerir.

## Dosyalar ve işletim

| Kaynak | Konum |
|---|---|
| Hizmet / CLI | `/usr/bin/hoodmin` / `/usr/bin/hoodmin-cli` |
| systemd hizmetleri | `hoodmin.service` (hoodmin), `hoodmin-upgrade.service` (root) |
| Yükseltme politikası / durum | `/etc/hoodmin/upgrade.json` (0600), `/var/lib/hoodmin-upgrade/state.json` |
| Yapılandırma | `/etc/hoodmin/hoodmin.json`, root:hoodmin 0640 |
| Ana anahtar | `/var/lib/hoodmin/master.key`, hoodmin:hoodmin 0600 |
| AutoSign dosyaları | `/var/lib/hoodmin/acme/` |
| İlk sahip bilgileri | `/root/hoodmin-first-owner.json`, 0600 |
| Statik arayüz | `/usr/share/hoodmin/web/` |
| Belgeler | `/usr/share/doc/hoodmin/docs/` |

Veritabanı adı ve rol, bağlantı dizesinden okunur. `hoodmin` sistem hesabı panel giriş hesabı değildir. Yerel soket ve uzak TLS seçenekleri aynı şema/kiracı korumasını kullanır.

Yapılandırma veya veritabanı erişilemiyorsa hizmet kapanmaz. Tarayıcı kısa bir hata gösterir; ayrıntı `journalctl -u hoodmin.service` içindedir. Bağlantı geri geldiğinde otomatik toparlanır. `/healthz` HTTP sürecinin çalıştığını, `/readyz` veritabanı/uygulamanın hazır olduğunu bildirir. Dinleme adresi değişikliği için hizmeti yeniden başlatın.

## Bakım ve kaynak paket

Paket güncellemeleri için [yükseltme kılavuzunu](yukseltme.md), kurtarma ve düzenli kopyalar için [yedekleme kılavuzunu](yedekleme.md) kullanın. Kaynaktan Debian paketi üretme adımları [geliştirme başvurusunda](dogrulama.md) yer alır.
