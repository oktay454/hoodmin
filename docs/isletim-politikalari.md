# Anahtarlar, yedek saklama ve bildirimler

Bu işlemler Hoodmin yönetim verisini korur. VPN trafiği veya VPN sunucusunun kullanıcı erişim anahtarları başka bir ürünün sunucu yönetimine dönüştürülmez.

## Ana anahtarı döndürme

1. Çalışan SSH/sertifika işlerinin bitmesini bekleyin ve bağımsız, doğrulanmış bir yedek alın.
2. Panel makinesinde root olarak anahtar kimliklerini kontrol edin:

```bash
hoodmin-cli keys status
hoodmin-cli keys rotate --confirmed
hoodmin-cli keys status
```

Döndürme (rotation), bütün kiracıların sunucu yapılandırmasını, agent kimliğini, erişim anahtarı sırlarını, oturum anahtarlarını, grup PFX verilerini, yayıncı ve bildirim hedefi sırlarını yeni AES-256-GCM anahtarıyla yeniden şifreler. VPN kimliği ve istemcilere dağıtılmış anahtarların içeriği değişmez. İşlem süresince şifreli veri işlemleri veritabanı kilidinde bekleyebilir; kısa süreli kullanılamama yanıtı mümkündür. Hizmet etkin anahtar değişimini algılayıp yeniden yükler.

Her bağımsız veritabanı kurulumu ayrı anahtar dosyası kullanmalıdır; aynı dosyayı farklı etkin kurulumlar arasında paylaşmayın. Anahtar dosyası 0600 izinli bir anahtar halkasıdır (keyring). Önce eski/yeni anahtarı içeren dosya kalıcı yazılır; sonra bütün şifreli alanlar ve etkin anahtar kimliği tek PostgreSQL işlemiyle güncellenir. Bozuk bir şifreli alan varsa veya bakım işlevi tüm kiracıları RLS sınırından bağımsız okuyamıyorsa işlem geri alınır. Dosyadaki etkin kimlik güncellenmeden kesinti olsa da veritabanındaki kimlik doğru anahtarı seçer. İşlemi tekrar çalıştırabilirsiniz; dosyayı elle değiştirmeyin.

Önceki anahtarlar eski yedeklerin kurtarılabilmesi için korunur. Halkada en fazla 32 anahtar bulunabilir; otomatik anahtar silme yoktur. Kesintiye uğramış bir denemenin kullanılmamış anahtarı da halkada kalabilir. Döndürme, ele geçirilmiş veritabanı ve anahtarın eski kopyasını geri alamaz. Eski kopyaları saklama politikanıza göre koruyun/imha edin. Sertifika özel anahtarları, CA hesapları, bot/parola veya VPN bağlantı kimlikleri bu komutla yenilenmez.

## Yedek saklama politikasını kurma

Önce [yedekleme ve kurtarma](yedekleme.md) adımlarını tamamlayın. Saklama varsayılan olarak kapalıdır. Root tarafından okunabilen 0600 bir JSON dosyası hazırlayın:

```json
{
	"enabled": true,
	"directory": "/var/backups/hoodmin",
	"keep_min": 3,
	"max_count": 30,
	"max_age_days": 30,
	"max_bytes": 10737418240,
	"min_free_bytes": 1073741824,
	"notify_tenant": null
}
```

```bash
hoodmin-cli backup policy --file /root/hoodmin-retention.json
hoodmin-cli backup policy
hoodmin-cli backup prune
hoodmin-cli backup prune --apply
```

`backup prune` yalnız önizlemedir. `--apply` silmeyi açıkça etkinleştirir. Politika açıkken kendi hedef dizinindeki başarılı `backup create/export` sonunda da uygulanır. Yedekleme zamanını ayrıca systemd zamanlayıcısıyla belirleyin; politika kendi başına yedek oluşturmaz.

Yalnız `hoodmin-…` dizinlerindeki tamamlanma kaydı ve bütün dosya ağacının SHA-256 özetleri ve sembolik bağlantı hedefleri doğrulanmış kopyalar sayılır. Yarım/bozuk, tanınmayan ve sembolik bağlı dizinler korunur ve kullanılabilir kopya sayısına katılmaz. Yeni kopyanın tamamlanma kaydı en son yazılır. Saklama; yaş, sayı, toplam bayt veya boş alan sınırı aşıldığında en eski uygun kopyadan başlar. En yeni `keep_min` doğrulanmış kopya ve `latest.age` dosyasının işaret ettiği son şifreli kopya korunur. Düz ve şifreli yedekler karışık alınıyorsa bu ek koruma sayı sınırının üstünde kopya bırakabilir. Bu sınırlarla politika sağlanamıyorsa disk baskısı bildirilir; korunan kopyalar silinmez.

`max_age_days:0` yaş sınırını, `max_bytes:0` toplam boyut sınırını kaldırır. `max_count`, `keep_min` değerinden az olamaz. Boyut sınırı doğrulanmış dizinleri ve ilişkili tarihli `.age` arşivlerini kapsar. `latest.age`, en yeni şifreli arşive atomik olarak bağlanır. Yarım ve tanınmayan dosyalar otomatik temizlenmediği için disk tüketimini ayrıca izleyin. Boş alan denetimi bir dosya sistemi rezervasyonu değildir; çalışmakta olan büyük dökümün diski doldurmasını garantiyle önlemez.

## Bildirim kanalı oluşturma

1. Yönetici/sahip hesabıyla **Bildirimler** bölümünü açın.
2. **Webhook ekle** veya **E-posta ekle** seçin.
3. Hedefi doldurun, etkinleştirip kaydedin.
4. **Sına** ile deneme iletisini sıraya alın ve **Son gönderimler** sonucunu kontrol edin.

Webhook, HTTPS POST ile JSON olay gönderir. İsteğe bağlı Bearer belirteci kullanılabilir. E-posta başlangıçtan itibaren TLS kullanan SMTP (SMTPS, çoğunlukla 465) ve kimlik doğrulaması kullanır. STARTTLS veya düz metin SMTP sunulmaz. Hedef genel ağda olmalı; DNS/IP ve TLS sertifikası doğrulanır. HTTP yönlendirmeleri takip edilmez; özel ağ hedefleri kabul edilmez. Hedef sırları veritabanında şifrelenir, listeleme yanıtında gösterilmez. Mevcut sırrı korumak için **Hedefi değiştir** kutusunu kapalı bırakın.

Olaylar: VPN sunucusu çevrimdışı/iyileşme, AutoSign başarısızlık/iyileşme ve harici yayım başarısızlık/iyileşme. Bakımda durdurulan sunucular için çevrimdışı bildirimi üretilmez. İşleyici durumu yaklaşık on saniyede bir kontrol eder; sunucunun çevrimiçi toleransı ayrıca uygulanır. Değişmeyen durum tekrar olay üretmez. Yeni kanal önceki olayları geriye dönük göndermez.

Kalıcı kuyruk (outbox) başarısız gönderimi üstel beklemeyle en fazla sekiz defa dener; bekleme üst sınırı bir saattir. Teslim, hedefin başarılı HTTP/SMTP yanıtıdır; e-postanın gelen kutusuna düşmesi garantisi değildir. Hedef kabul ettikten hemen sonra kesinti olursa aynı olay tekrar gelebilir: olay `id` değerini tekilleştirin. Kanalı devre dışı bırakmak bekleyen gönderimleri iptal eder; başlamış ağ aktarımı geri alınamaz. Tamamlanmış kayıtlar otuz gün tutulur; bekleyenler korunur.

İletide kiracı, kaynak kimliği, olay kimliği, durum ve UTC zamanı bulunur. VPN anahtarları, PFX, hedef sırları ve ayrıntılı sunucu günlükleri gönderilmez.

### İşletmen yedek bildirimleri

Panelin yedek/disk olaylarını alacak çalışma alanının UUID'sini politika dosyasındaki `notify_tenant` alanına yazın. Bu tercih yalnız panel makinesinin root işletmenince yapılır; diğer kiracılara sistem bilgisi açılmaz. O çalışma alanında etkin kanal oluşturun.

CLI yedek işlemleri özel `backup-status.json` dosyasına başarı/disk durumunu kaydeder. İşleyici bu sonucu belirtilen kiracının kanalına hata/iyileşme olayı olarak aktarır. Durum, son CLI yedek işleminin sonucudur; bağımsız sürekli disk taraması değildir. PostgreSQL veya panel tamamen durmuşken bu işleyici bildirim gönderemez. `/readyz`, disk ve yedek zamanlayıcısını bağımsız izleme sisteminizle de izleyin.

## CLI/API ile bildirimler

`hoodmin-cli api get /notifications` kanalları ve son yüz gönderimi döndürür. Yönetici rolü gerekir. `/notifications/{UUID}` yoluna `name`, `enabled`, mevcutsa `version`, `target`, `confirmed:true` gönderin. Yeni UUID'yi istemci üretir. `target:null` mevcut şifreli hedefi korur.

Webhook hedefi:

```json
{"kind":"webhook","url":"https://alerts.example.com/hoodmin","bearer":"HEDEF_SIRRI"}
```

SMTPS hedefi:

```json
{"kind":"smtp","host":"mail.example.com","port":465,"username":"hoodmin","password":"SMTP_SIRRI","from":"vpn@example.com","to":"operator@example.com"}
```

`POST /notifications/{UUID}/test`, `confirmed:true` ile sınama olayı oluşturur. İnsan oturumunda CLI, CSRF/Origin bilgilerini kendisi ekler. Bir kiracıya en fazla sekiz kanal eklenir. Sırlar içeren istek dosyalarını 0600 izinle saklayın.
