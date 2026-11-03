# hoodmin-cli

`hoodmin-cli`, Rust ile yazılmış kurulum, bakım ve VPN yönetim aracıdır. Hizmet `hoodmin`, araç `hoodmin-cli` olarak çalışır. Ayrı Python/Bash yönetim yardımcıları gerekmez. CLI yardımını veritabanı yapılandırılmadan da açabilirsiniz:

```bash
hoodmin-cli --help
hoodmin-cli config --help
hoodmin-cli database --help
hoodmin-cli api --help
hoodmin-cli backup --help
```

## Kurulum ve yerel bakım

[Kurulum kılavuzundaki](uretim-kurulumu.md) sırayla:

```bash
hoodmin-cli config set --database-url-file /root/hoodmin-db.url --public-origin https://hoodmin.example.com
hoodmin-cli config show
hoodmin-cli database init --local
hoodmin-cli owner-init --email vpn@example.com
hoodmin-cli service enable
hoodmin-cli service start
```

Uzak PostgreSQL için `database init --local` yerine `database init --admin-url-file /root/hoodmin-db-admin.url` kullanılır. `database check` uygulama rolü ve şemayla bağlantıyı kontrol eder. Yerel sokette bu kontrolü hizmet hesabıyla çalıştırın:

```bash
runuser -u hoodmin -- hoodmin-cli database check
```

Yapılandırma varsayılan `/etc/hoodmin/hoodmin.json` dosyasındadır; `--config DOSYA` ile değişir. `config show` bağlantı parolasını gizler. `config set`, veritabanı henüz hazır olmadan ayarı kaydedebilir. Hizmet hazır olana kadar tarayıcı basit bir durum sayfası gösterir.

`service start|stop|restart|status|enable|disable` yerel systemd hizmetini yönetir. Değişiklikler root gerektirir.

## Web işlemlerini API üzerinden çalıştırma

CLI yönetim isteklerini **aynı sunucu işlevlerine** yollar. Kiracı, rol, onay, sürüm ve denetim kaydı kuralları web arayüzündeki gibidir. CLI bir rol yükseltme yolu değildir.

İnsan hesabı için parolayı 0600 özel dosyaya yazın:

```bash
hoodmin-cli api --origin https://hoodmin.example.com login --email vpn@example.com --password-file /OZEL_YOL/parola.txt
hoodmin-cli api whoami
hoodmin-cli api get /servers
hoodmin-cli api get /dashboard
hoodmin-cli api get /views/audit
hoodmin-cli api get /reports?hours=24
```

Oturum varsayılan `$HOME/.config/hoodmin/session.json` içinde 0600 izinle saklanır. Ayrı hesap/alan adı için `--session-file DOSYA` kullanın. Oturum başka bir adrese gönderilmez. Yazma isteklerinde CLI, Origin ve CSRF değerlerini otomatik ekler. Parola değişimi ve oturum süresi sonu tekrar giriş gerektirir.

Bot için panelden uygun rol/süreyle oluşturulan anahtarı 0600 dosyada tutun:

```bash
hoodmin-cli api --origin https://hoodmin.example.com --token-file /OZEL_YOL/bot.txt get /servers
```

Botlar yalnız verilen rolün işlemlerini yapar; sahip rolü alamaz. İnsan hesabının sahip işlemleri kendi doğrulanmış oturumuyla kullanılabilir.

### JSON değişikliği gönderme

Örneğin `/root/sunucu.json`:

```json
{
	"name": "VPN İstanbul",
	"host": "vpn.example.com",
	"port": 443,
	"farm": ""
}
```

```bash
hoodmin-cli api post /servers --data-file /root/sunucu.json --output /root/sunucu-baglantisi.json
```

`--output` yeni 0600 dosya oluşturur; var olan dosyanın üzerine yazmaz. Oluşturma yanıtı sunucuya ait sırlar içerebilir. `--output` kullanılmazsa yanıt standart çıktıya gider; sır içeren yanıtları günlük veya herkese açık terminale yönlendirmeyin.

Gerekli `confirmed:true`, `version`, `base_code` veya kimlik yenileme kanıtı JSON dosyasında açıkça verilmelidir. CLI bunları kendiliğinden onaylamaz. Hedef yollar ve alanlar [API başvurusunda](yonetim-api.md) bulunur. Anahtarlar, profiller, gruplar, PFX, AutoSign, harici yayım, ekip/botlar ve oturum işlemleri aynı `api get/post` düzenini kullanır.

CSV veya anahtar dosyası:

```bash
hoodmin-cli api get /usage.csv --output /root/kullanim.csv
hoodmin-cli api get /tokens/ANAHTAR_UUID/download --output /root/erisim.vh
hoodmin-cli api logout
```

## Yedekleme

```bash
hoodmin-cli backup create
hoodmin-cli backup export
```

Yerel sokette yönetici dökümü yerel `postgres` hesabından alınır. Uzak sunucuda uygulama rolü RLS atlayamadığı için `--admin-url-file` ile **uygulamanın veritabanını hedefleyen** yedek yetkili bağlantı verilmelidir. `pg_dump`, şifreli dışa aktarımda ayrıca `age` gerekir. Bu araçları komut kendiliğinden kurmaz. [Yedekleme ve kurtarma](yedekleme.md).

`backup pull`, sabit SSH komutuyla sınırlandırılmış bir hesaptan şifreli dosyayı alır; sunucu parmak izi ve kimlik dosyası açıkça verilir. `--help` seçenekleri gösterir.

## Düşük seviyeli komutlar

`create-account`, `register-server`, `create-token`, `revoke-token` ve dahili Certbot `acme-hook` komutları da aynı Rust CLI'dedir. Bunlar yerel/veritabanı düzeyindeki güvenilir işletmen araçlarıdır. Günlük ekip otomasyonu için web ile aynı yetki denetimini kullanan `api` komutlarını tercih edin. Sistem hesabı ve ana anahtara erişim, panel üye yetkisinden farklı bir işletmen yetkisidir.

## Anahtarlar, saklama ve bildirimler

`keys status`, `keys rotate --confirmed`, `backup policy [--file DOSYA]` ve `backup prune [--apply]` root işletim komutlarıdır. Anahtar döndürmeden önce doğrulanmış bağımsız yedek alın. Bildirim kanalları web ile aynı `/notifications` API yollarıyla yönetilir. Alanlar, kesinti davranışı ve sınırlar: [işletim politikaları](isletim-politikalari.md).

## Statik arayüz sunumu

```bash
hoodmin-cli config web --serve-static true
hoodmin-cli config web --serve-static false
hoodmin-cli config show
```

`true` Hoodmin’in `/usr/share/hoodmin/web/` dosyalarını sunmasını sağlar; `false` Nginx doğrudan sunumu içindir. Aynı makinede kopyalama gerekmez, ayrı makinede yalnız bu ağaç kopyalanır. İsteğe bağlı `--directory /MUTLAK/YOL` farklı statik kök seçer. `config set` bu tercihi korur. [Nginx yapılandırmaları](web-sunumu.md).
