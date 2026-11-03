# Arayüzü Hoodmin veya Nginx ile sunma

Hoodmin tek Debian paketiyle iki sunum biçimini destekler. İkisi de aynı istemci taraflı uygulamayı (SPA) kullanır; bütün ekranlar tarayıcıda oluşturulur. Yönetim API'si `/api/manage`, VPN sunucusu protokolü `/api/agent` altındadır. Tarayıcı ve API aynı genel HTTPS alan adını kullanır; ayrı CORS veya ikinci oturum yapılandırması gerekmez.

Arayüz adresleri `/`, `/servers`, `/access-keys`, `/sessions`, `/account` biçimindedir. Sunucu ayrıntısı `/servers/KIMLIK/configuration` olur. Adreslere `/panel` veya `/manage` eklemeyin.

## Tek statik dizin

Paket, aşağıdaki ağacı root sahipliğinde ve herkesçe okunabilir kurar. Bu dizinde yapılandırma, parola, oturum veya ana anahtar bulunmaz:

```text
/usr/share/hoodmin/web/
├── index.html
└── assets/
    ├── app.css
    ├── spa.js
    └── locales/
        ├── tr.json
        ├── en.json
        └── ar.json
```

Hoodmin de Nginx de bu dosyaları okur. İkili dosya içine gömülü ikinci arayüz yoktur. Nginx ayrı makinedeyse **yalnız bu ağacı** kopyalayın; Hoodmin paketini, CLI'ı, yapılandırmayı veya veritabanını Nginx makinesine kurmanız gerekmez. Arayüz ve arka uç aynı Hoodmin sürümüne ait olmalıdır.

## 1. Hoodmin statikleri sunar

Varsayılan ayar:

```json
"web": {
	"serve_static": true,
	"directory": "/usr/share/hoodmin/web"
}
```

```bash
hoodmin-cli config web --serve-static true
hoodmin-cli config show
```

`deploy/nginx-hoodmin.conf` örneğini kullanın: Nginx bütün istekleri loopback üzerindeki Hoodmin'e iletir. Dosya kopyalamaya gerek yoktur. Veritabanı kullanılamazken Hoodmin arayüzü sunmaya devam eder; API kısa hata döndürür. Hoodmin sürecinin kendisi durmuşsa bu biçimde arayüz de sunulamaz.

## 2. Aynı makinedeki Nginx statikleri sunar

1. [Temel kurulumu](uretim-kurulumu.md) tamamlayın. HTTPS sertifikasını hazırlayın.
2. `deploy/nginx-hoodmin-static.conf` örneğini `/etc/nginx/sites-available/hoodmin` konumuna koyun. `server_name`, HTTP yönlendirmesi ve sertifika yollarını kendi alan adınıza göre değiştirin. Aynı alan adı için önceki yapılandırmayı devre dışı bırakın; örnekler alternatiflerdir.
3. Nginx'in `/usr/share/hoodmin/web` dizinini okuyabildiğini kontrol edin; yazma yetkisi vermeyin.
4. Aşağıdaki komutları çalıştırın:

```bash
nginx -t
systemctl reload nginx
hoodmin-cli config web --serve-static false
```

Dosya kopyalanmaz. Nginx doğrudan paketin dizininden okur. Hoodmin yalnız API ve dinamik VPN adres belirteçlerini sunar. Paket yükseldiğinde arayüz dosyaları da yükselir.

## 3. Ayrı makinedeki Nginx statikleri sunar

Yerleşim:

```text
Tarayıcı → genel Nginx (statik dosyalar)
                    └→ HTTPS → arka uç Nginx → loopback Hoodmin
```

1. Arka uç makinede Hoodmin'i normal kurun. `public_origin`, tarayıcının kullandığı **genel** HTTPS adresi olmalıdır; arka uç adresi değildir.
2. Arka uç için Nginx'in erişebildiği bir DNS adı ve bu adı kapsayan HTTPS sertifikası hazırlayın. `deploy/nginx-hoodmin-backend.conf` örneğinde DNS/sertifika yollarını ve `allow 192.0.2.10` satırını genel Nginx'in gerçek çıkış IP'siyle değiştirin. Arka uca ağ erişimini de bu makineyle sınırlandırın. Bu örnek, istemci IP başlığını yalnız izin verilen ön uçtan kabul eder.
3. Genel Nginx makinesinde statik dizini kopyalayın. Örnekteki adresi ve sürümü kendi kurulumunuzla eşleştirin. Komutları genel Nginx makinesinde root olarak çalıştırın:

```bash
install -d -m 0755 /usr/share/hoodmin
scp -r root@ARKA_UC:/usr/share/hoodmin/web /usr/share/hoodmin/web-0.5.0
ln -s web-0.5.0 /usr/share/hoodmin/web
```

Kopyanın hedef sürüm dizini önceden mevcut olmamalıdır. Bu makinede Hoodmin paketi gerekmez. SSH kopyasında yalnız herkese açık statik dosyalar taşınır; `/etc/hoodmin` veya `/var/lib/hoodmin` kopyalanmaz.

4. Genel makinede `deploy/nginx-hoodmin-static-remote.conf` örneğini kullanın. Genel alan adı/sertifikaları ve arka uç DNS adını değiştirin. `proxy_ssl_name` arka uç sertifikasındaki adla eşleşmelidir. Varsayılan CA deposu genel sertifikalar içindir; özel CA kullanılıyorsa `proxy_ssl_trusted_certificate` için yalnız açık CA dosyasını gösterin. Sertifika doğrulamasını kapatmayın.
5. İki Nginx'te de `nginx -t` başarılı olduktan sonra yeniden yükleyin. Arka uçta `hoodmin-cli config web --serve-static false` çalıştırın. Arayüz dosyaları artık Hoodmin tarafından sunulmaz.
6. Genel adreste giriş, sunucu ayrıntısı ve API işlemini sınayın. Başka bir arka uç adresi için ikinci bir tarayıcı oturumu veya CORS açmayın.

Yükseltmede yeni sürümü yeni dizine kopyalayın. Önce eşleşen arka uç sürümünü hazırlayın; sonra genel makinede bağlantıyı tek adımda değiştirin:

```bash
scp -r root@ARKA_UC:/usr/share/hoodmin/web /usr/share/hoodmin/web-YENI_SURUM
ln -s web-YENI_SURUM /usr/share/hoodmin/web.next
mv -Tf /usr/share/hoodmin/web.next /usr/share/hoodmin/web
```

Bu örnekte `web` ilk kurulumdan beri sembolik bağlantıdır. Eski sürüm dizinini bakım sonuna kadar saklayabilirsiniz. Tarayıcı yenilendiğinde yeni dosyaları alır; dosyalara uzun süreli değişmez önbellek uygulanmaz.

## Dinamik yollar ve kesinti davranışı

Nginx örnekleri aşağıdaki yolları arka uca iletir:

- `/api/...`: yönetim, agent, kancalar ve `/api/manage/events` olay akışı.
- `/s/...`, `/f/...`: istemcilere dağıtılan şifreli VPN adres belirteçleri; bunlar statik dosya değildir.
- `/healthz`, `/readyz`: süreç ve uygulama hazırlığı.

Yalnız bilinen arayüz yolları `index.html` alır; doğrudan ayrıntı sayfasını açmak ve yenilemek çalışır. Eksik dosya veya bilinmeyen API yolu HTML iskeletine dönüştürülmez. Hoodmin hazır değilken API 503, durduğunda Nginx 502/504 döndürebilir; statikleri Nginx sunuyorsa arayüz görünür kalır, basit hata gösterir ve yeniden dener. `/healthz` ve `/readyz` statik bir başarı sayfası olarak taklit edilmez.

Her iki biçimde oturum çerezleri HttpOnly/Secure, kaynak doğrulaması (Origin) ve CSRF koruması korunur. Nginx örnekleri CSP, MIME ve önbellek başlıklarını da uygular. Örnekler Nginx'in standart `http` bağlamında, `/etc/nginx/mime.types` yüklenmiş olarak kullanılır. Farklı statik konum gerekiyorsa Nginx `root` ve Hoodmin'in `web.directory` alanını eşleştirin; CLI karşılığı `config web --serve-static true --directory /MUTLAK/YOL` olur.

Örneklerin kurulu kopyaları `/usr/share/doc/hoodmin/examples/` altındadır. Web sunum ayarı veritabanı erişiminden bağımsız okunur; çalışan hizmet normalde birkaç saniye içinde değişikliği alır. Dinleme adresi değişikliği yeniden başlatma gerektirir. `hoodmin-cli config set` mevcut web sunum tercihini korur.
