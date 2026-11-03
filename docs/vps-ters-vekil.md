# Ayrı VPS üzerinden HTTPS ters vekil

Standart kurulum, [aynı makinedeki Nginx](uretim-kurulumu.md#5-nginx-ters-vekilini-etkinleştirme) ile daha kolaydır. Bu başvuru, genel TLS vekilinin ayrı VPS'te bulunduğu yerleşimler içindir.

Bu yerleşimde statikleri de genel VPS’ten sunmak için [ayrı Nginx makinesi kurulumu](web-sunumu.md#3-ayrı-makinedeki-nginx-statikleri-sunar) ve hazır yapılandırmaları kullanın. Yalnız `/usr/share/hoodmin/web/` ağacı kopyalanır; Hoodmin loopback dinleyicisi doğrudan internete açılmaz.

## Bağlantı düzeni

Tarayıcı/agent → genel VPS Nginx → arka uç Nginx → `127.0.0.1:3080` Hoodmin.

Her HTTPS bağlantısında sertifika ve ad doğrulanır. Arka uç için özel CA kullanılıyorsa yalnız o vekil bağlantısına CA dosyası tanımlayın; özel CA anahtarını VPS'e taşımayın. `proxy_ssl_verify off` kullanmayın.

## Kurulum adımları

1. Panel DNS'ini VPS'e yönlendirin ve bu adı kapsayan genel TLS sertifikası hazırlayın.
2. VPS'ten arka uç IP/portuna erişimi açın. Arka uç sertifikasının doğrulanacak DNS adını belirleyin; bağlantı IP'si ile sertifika adı farklı olabilir.
3. VPS'in HTTPS konumunda aşağıdaki başlık/TLS düzenini kendi adreslerinize uyarlayın:

```nginx
location / {
	proxy_pass https://ARKA_UC_IP:443;
	proxy_ssl_server_name on;
	proxy_ssl_name backend.example.com;
	proxy_ssl_verify on;
	proxy_ssl_trusted_certificate /etc/nginx/hoodmin-backend-ca.crt;
	proxy_set_header Host $host;
	proxy_set_header X-Real-IP $remote_addr;
	proxy_set_header X-Forwarded-For $remote_addr;
	proxy_set_header X-Forwarded-Proto https;
	proxy_set_header Connection "";
	proxy_http_version 1.1;
	proxy_buffering off;
	proxy_read_timeout 3600s;
}
```

4. Arka uç Nginx'te **yalnız VPS'in gerçek çıkış IP'sini** güvenilir vekil olarak tanımlayın:

```nginx
set_real_ip_from VPS_CIKIS_IP/32;
real_ip_header X-Real-IP;
```

Arka uçtan Rust'a `X-Forwarded-For $remote_addr` yazılmalıdır. İstemcinin kendi başlığını aynen geçirmeyin; tüm interneti güvenilir vekil saymayın.

5. `/etc/hoodmin/hoodmin.json` içindeki `public_origin` değerini genel HTTPS adresiyle eşleştirip Hoodmin'i yeniden başlatın.
6. Her Nginx'te `nginx -t` başarılı olduktan sonra yeniden yükleyin. Genel `/healthz`, giriş/form gönderimi ve `/api/manage/events` olay akışını kontrol edin.

Genel ve arka uç sertifikalarının ikisinin de yenilemesini izleyin. WAF kullanılıyorsa agent ve yükleme isteklerinin gerçek içerik türlerini koruyun; şifreli düz metni otomatik JSON'a dönüştürmeyin. Ürünün standart veri biçimini vekil güvenlik kuralına göre değiştirmeyin.

Alan adı değişince panel oturumu yeniden açılır. Bağlı VPN sunucularının yönetim kök adreslerini de güncelleyin; çerez ve agent yapılandırması kendiliğinden başka alan adına taşınmaz.
