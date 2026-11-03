# Hoodmin yükseltme kılavuzu

Hoodmin paket güncellemesi, yapılandırma ve veritabanı geçişlerini ayrı bir yönetici hizmetiyle tamamlar. `hoodmin.service` uygulama hesabıyla çalışır; şema veya sistem yetkisi gerektiren yükseltme işlemi yapmaz. SQL komutları Rust PostgreSQL sürücüsüyle (SQLx) gönderilir; uygulama `psql` çalıştırmaz.

## İlk hazırlık

İlk kurulumda [bağlantı, veritabanı ve sahip hesabını](uretim-kurulumu.md) açıkça hazırlayın. Otomatik yükseltme bunları kendiliğinden oluşturmaz. Uygulama rolünü süper kullanıcı veya BYPASSRLS yapmayın.

Bekleyen geçiş uygulanmadan önce tam yedek alınır. Hoodmin makinesinde veritabanı sürümüne uygun `pg_dump` sağlayan Debian `postgresql-client` paketi ve yeterli disk alanı bulunmalıdır. PostgreSQL sunucusunun aynı makinede olması gerekmez. Araç eksikse yardımcı hizmet hata kaydeder ve geçişi uygulamaz; paket içinde bağımlılık kurulumu çalıştırılmaz. Şifreli bağımsız yedeğinizi de ayrıca saklayın; yerel yükseltme yedeği disk kaybına karşı koruma sağlamaz.

### Yerel PostgreSQL

Uygulama bağlantısı Unix soketini kullanıyorsa yardımcı hizmet yerel `postgres` hesabına geçerek eş kimliğiyle (peer authentication) bağlanır. Ek yönetici parolası gerekmez. Uygulama soket dizini, port ve veritabanı adı kullanılır.

### Uzak PostgreSQL

Uygulama bağlantısından ayrı bir yönetici bağlantı dizesini editörle `/root/hoodmin-upgrade-admin.url` dosyasına yazın. **Uygulamayla aynı sunucu, port ve uygulama veritabanını** hedeflemelidir. İlk `database init` için kullanılan `/postgres` yönetim veritabanından farklı olarak burada örneğin `/hoodmin` gerekir:

```text
postgresql://SEMA_YONETICISI:PAROLA@db.example.com:5432/hoodmin?sslmode=verify-full&sslrootcert=/etc/hoodmin/db-ca.crt
```

```bash
chown root:root /root/hoodmin-upgrade-admin.url
chmod 0600 /root/hoodmin-upgrade-admin.url
hoodmin-cli upgrade --configure --admin-url-file /root/hoodmin-upgrade-admin.url
```

Dosya normal, tek bağlantılı ve sembolik bağlantı olmayan root dosyası olmalıdır. Dizeyi komut argümanına veya kabuk geçmişine yazmayın. Yönetici rolü şema geçişleri, RLS korumalı yedek ve güvenlik tanımlayıcılı bakım işlevleri için SUPERUSER veya BYPASSRLS ve gereken DDL yetkilerini taşımalıdır. Özel CA dosyası kök hizmetin de okuyabildiği kalıcı konumda olmalıdır. `verify-full` hem CA'yı hem DNS adını doğrular; doğrulama kapatılamaz. Web hesabı yönetici dosyasını okuyamaz. Dosyayı kalıcı konumda tutun; systemd özel geçici dizini (PrivateTmp) nedeniyle `/tmp` ve `/var/tmp` altındaki işletmen dosyaları yardımcı hizmetten görünmeyebilir.

## Paket güncellemesi ve başlangıç

```bash
apt install ./hoodmin_YENI_SURUM_amd64.deb
hoodmin-cli upgrade --status
```

1. Önceden çalışan panelin durumu kaydedilir; paket değiştirilirken panel ve eski yükseltme yardımcısı durdurulur.
2. Kurulum sonrası betiği (postinst), root yetkili `hoodmin-upgrade.service` hizmetini başlatır. Debian `policy-rc.d` hizmet başlatma politikası korunur.
3. Paket en fazla 20 saniye durum bekler. Geçici bağlantı sorunu varsa paket başarılı kurulur; yardımcı hizmet bekleyen işi sürdürür. Kalıcı hata görülmüşse kurulum hata bildirir; önceden çalışan HTTP hizmeti yine açılır ve API hazırlığı kapalı kalır. Sonradan oluşan hata yardımcı hizmetin durumunda ve günlüğünde görünür.
4. Önceden çalışan panel yeniden başlatılır. Yönetici tarafından durdurulmuş panel açılmaz; etkinleştirme (enable) tercihi değiştirilmez.
5. Her `hoodmin.service` başlangıcı yardımcı hizmeti tekrar tetikler. İki hizmet paralel başlar; HTTP süreci yükseltmeyi beklerken kapanmaz. API, eşleşen geçişler ve hazır durum doğrulanana kadar 503 döndürür. Arayüz kısa mesaj gösterir ve tekrar dener.

Yardımcı hizmet tamamlandığında normal olarak çıkar; bu nedenle `inactive (dead)` tek başına hata değildir. Sonucu `hoodmin-cli upgrade --status` içindeki `phase` alanından okuyun. Veritabanı veya yapılandırma sorunu giderilince bekleyen hizmet otomatik dener; hemen denemek için:

```bash
systemctl restart hoodmin-upgrade.service
journalctl -u hoodmin-upgrade.service --no-pager
curl --fail --show-error https://hoodmin.example.com/readyz
```

## Durumlar ve ayarlar

| Durum (`phase`) | Anlamı |
|---|---|
| `needs_setup` | Veritabanı bağlantısı henüz yapılandırılmamış. İlk kurulum adımlarını tamamlayın. |
| `pending` / `running` | Denetim veya yükseltme sürüyor; başka yerel bakım işleminin kilidi bekleniyor olabilir. |
| `waiting_database` | Geçici bağlantı, süre aşımı veya veritabanı bakım kilidi nedeniyle yeniden denenecek. |
| `needs_admin` | Uzak yönetici dosyası eksik, izinleri veya hedefi yanlış. |
| `failed` | Geçiş özeti, izin, TLS doğrulaması, yedek veya başka bir kalıcı hata. Günlüğü inceleyin. |
| `ready` | Bu paket sürümü, geçiş kataloğu ve bağlantı yapılandırması için yükseltme denetimi tamamlanmış. |

`ready` canlı veritabanı erişiminin sürekli garantisi değildir; `/readyz` ayrıca uygulama bağlantısını ve şemayı denetler. Sonradan oluşan bağlantı kesintisinde HTTP ayakta kalır, API erişimi durur; bağlantı gelince toparlanır.

Root sahipliğinde 0600 `/etc/hoodmin/upgrade.json`:

```json
{
	"admin_url_file": null,
	"backup_directory": "/var/backups/hoodmin-upgrade",
	"attempt_timeout_seconds": 300,
	"retry_seconds": 30
}
```

`admin_url_file` yerel sokette `null`, uzak sunucuda yönetici dosyasının mutlak yoludur. `attempt_timeout_seconds` yedek veya uygulama alt işleminin üst sınırıdır (1–3600 saniye). Ön bağlantı denetiminin üst sınırı 10 saniyedir; SQL bildirim süresi 240, satır/nesne kilidi beklemesi 3 saniyedir. `retry_seconds` denemeler arası beklemedir (1–300 saniye). Bu süreler paket betiğinin 20 saniyelik beklemesinden bağımsızdır. Süre aşımında alt işlem ve ona bağlı `pg_dump` süreçleri sonlandırılır.

systemd koruması yazmaya yalnız `/etc/hoodmin`, `/var/lib/hoodmin`, `/var/lib/hoodmin-upgrade` ve `/var/backups` altında izin verir. Yedek dizinini `/var/backups` altında tutun. Özel anahtar/veri dizini kullanıyorsanız yardımcı hizmetin yazma kapsamını da buna göre bilinçli olarak düzenleyin. Yönetici dosyaları salt okunur olabilir. Genel bağlantı ve sır ayrıntıları web yanıtına veya durum dosyasına yazılmaz; root günlüğünde parolalar gizlenir.

## Kilit, yedek ve yarıda kalma

Aynı makinedeki yükseltme, anahtar döndürme ve yedek işlemleri ana anahtarın yanındaki işletim sistemi dosya kilidini paylaşır. İşlem ölünce kilit otomatik bırakılır; boş kilit dosyasını silmek gerekmez. Veritabanında danışma kilitleri (advisory locks), farklı bağlantıların şema ve anahtar bakımını eşgüdümler. Uygulanmış her geçişin adı ve SHA-256 özeti `_hoodmin_migrations` tablosunda tutulur; geçiş ile kayıt tek işlemde (transaction) tamamlanır.

Yükseltme yedeği; veritabanı dökümünü, ana anahtarı, yapılandırmayı, mevcut ACME/Let's Encrypt dosyalarını ve bütünlük envanterini içerir. Root'a özel dizindedir. `upgrade-provenance.json` kaynak/hedef sürümü ve şema planını tanımlar; `package-version.txt` yedek anında kurulu paketi gösterir. Paket o sırada yenilenmiş olabileceği için eski sürüm kanıtı kaynak sürüm alanıdır. Geçiş öncesinde yedeğin bütünlüğü doğrulanır ve kalıcı kontrol noktası (checkpoint) yazılır.

Kesinti sonrasında aynı şema planı, yapılandırma ve anahtar hâlâ eşleşiyorsa doğrulanmış yedek yeniden kullanılır. Tamamlanan geçişler tekrarlanmaz. Eksik yedek tamamlanmış sayılmaz; bilinmeyen, sırası bozuk veya özeti değişmiş geçişlerde API açılmaz. Yükseltme tamamlandıktan sonraki olağan yeniden başlatma yeni yedek veya tekrarlanan şema işlemi üretmez.

## Sorun giderme ve geri dönüş

Yönetici dosyasını, bağlantıyı, disk alanını veya eksik `pg_dump` aracını düzelttikten sonra yardımcı hizmeti yeniden başlatın. Durum/izleme dosyalarını ve geçiş kayıtlarını silerek hatayı gizlemeyin. Yönetici dosyası ve şema kayıtlarının doğruluğundan emin olmadan `database init` ile yükseltmeyi zorlamayın.

Geçici kesinti için `dpkg --configure -a` zorunlu değildir; paket zaten tamamlanmıştır. Kalıcı hata nedeniyle paket yapılandırması yarıda kalmışsa sorunu giderip `dpkg --configure -a` çalıştırın. Otomatik şema düşürme yoktur. Geri dönüş için [bağımsız doğrulanmış yedekten kurtarma](yedekleme.md) uygulayın; eski paketle yeni veritabanını rastgele eşleştirmeyin. Ayrı Nginx makinesinde [aynı sürümün statik ağacını](web-sunumu.md) da güncelleyin.
