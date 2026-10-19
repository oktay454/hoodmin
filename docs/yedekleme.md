# Yedekleme ve kurtarma

Hoodmin'in veritabanı, ana anahtarı ve sertifika dosyaları birlikte korunmalıdır. Ana anahtar kaybolursa şifreli sunucu ayarları ve yayıncı kimlikleri kullanılamaz. Boş bir `master.key` üretmek kurtarma değildir.

## 1. Yerel yedek alma

Panel makinesinde root olarak:

```bash
hoodmin-backup
```

Komut, `/var/backups/hoodmin/` altında 0700 izinli yeni dizin oluşturur ve yolunu yazdırır. İçerik:

| Dosya/dizin | İçerik |
|---|---|
| `database.dump` | PostgreSQL özel biçimli veritabanı dökümü; şema, hesaplar ve kiracı kayıtları. |
| `master.key` | Şifreli yönetim verilerini açan ana anahtar. |
| `hoodmin.env`, `configuration/` | Ortam ve `/etc/hoodmin` ayarları. |
| `acme/` | Varsa AutoSign hesapları, özel anahtarlar, sertifikalar ve Certbot çalışma dosyaları. |
| `letsencrypt/` | Varsa panel/işletmen Certbot ayarları ve sertifikaları. |
| `package-version.txt` | Kurtarma için paket sürümü. |
| `SHA256SUMS` | Temel dosyalar ve ACME ağacının dosya özetleri. |

Yedek sır içerir; herkese açık dizinde tutmayın. Aynı diskteki kopya disk kaybına karşı korumaz. Sertifika/ayar değişikliği sırasında dosya ve veritabanı farklı anlara ait olabilir; tutarlı bakım kopyası için çalışan işleri tamamlayıp hizmeti durdurarak yedek alın veya bütün kapsayıcının durdurulmuş kopyasını kullanın.

## 2. Şifreli dış kopya

Bu seçenek Debian'ın `age` paketini gerektirir. Alıcı/kurtarma anahtar çiftini bağımsız, güvenli bir makinede `age-keygen` ile üretin. Özel kurtarma anahtarını panelde veya yedeğin içinde tutmayın. Açık `age1…` alıcı değerini panelde `/etc/hoodmin/backup-recipient.txt` dosyasına yazın.

```bash
hoodmin-backup-export
```

Komut yeni yerel yedek oluşturur ve alıcıya şifrelenmiş `/var/backups/hoodmin/latest.age` dosyasını atomik değiştirir. Bu dosyayı ayrı makineye kopyalayın. Uzak makinenin yalnız şifreli dosyayı okuyabilmesi için kaynak IP, sabit SSH komutu ve sunucu parmak iziyle sınırlandırılmış bir anahtar kullanın.

Zamanlayıcı örnekleri `/usr/share/doc/hoodmin/examples/` altındadır. İnceleyip kendi hedeflerinize uyarlayın; kendiliğinden kurulmaz/etkinleşmez. Yerel anlık görüntüler ve uzak kopyalar otomatik silinmez. Saklama süresi, disk alarmı ve eski yedek temizliği işletmenin sorumluluğundadır.

## 3. Yedeği doğrulama

Özel bir çalışma dizininde, şifreli arşivi ayrı saklanan kurtarma kimliğiyle açın:

```bash
umask 077
install -d -m 0700 /root/hoodmin-recovery
age --decrypt --identity /GUVENLI_YOL/kurtarma.agekey --output /root/hoodmin-recovery/backup.tar /GUVENLI_YOL/latest.age
tar -tf /root/hoodmin-recovery/backup.tar
```

Arşivin güvenilir kaynaktan geldiğini ve yollarını kontrol edin; sonra yine özel dizine açın. İçerikteki `SHA256SUMS` dosyasını `sha256sum --check SHA256SUMS` ile doğrulayın. Özet dosyası her ayar dosyasını kapsamaz; yalnız özet kontrolünü tam kurtarma sınaması saymayın.

## 4. Ayrı veritabanında kurtarma sınaması

Canlı veritabanının üzerine yazmayın. Aynı PostgreSQL ana sürümüyle, `hoodmin` rolünün hazır olduğu ayrılmış Hoodmin makinesinde:

```bash
runuser -u postgres -- createdb hoodmin_restore_check
runuser -u postgres -- pg_restore --exit-on-error --dbname=hoodmin_restore_check < /OZEL_YEDEK_DIZINI/database.dump
runuser -u postgres -- psql -X -d hoodmin_restore_check -c 'SELECT count(*) FROM accounts; SELECT count(*) FROM servers; SELECT count(*) FROM _hoodmin_migrations;'
```

Dosya root'a özel olduğu için döküm kabuk tarafından standart girdiye açılır; `postgres` kullanıcısına yedek dizinini açmanız gerekmez. İşlemleri boş hedefte çalıştırın; hata verirse incelemeden tekrar yüklemeyin.

Hesap/sunucu sayıları, geçiş kayıtları, kiracı RLS politikaları ve yedekteki ana anahtarla şifreli verilerin açılması denetlenmelidir. Ayrı deneme örneğini genel ağa veya gerçek VPN sunucularına bağlamayın; AutoSign/yayıncı çalışanları dış etki oluşturabilir.

## 5. Yerine geçecek kurulumu hazırlama

1. Kurtarma planında eski sürüme uygun paketi ve PostgreSQL sürümünü belirleyin. Orijinal hizmeti durdurun; aynı kiracıyı yöneten iki etkin panel bırakmayın.
2. Yerine geçecek ayrılmış makinede paketi kurun. İlk sahip oluşturma komutunu çalıştırmayın; hesaplar yedekten gelir.
3. Boş `vpnhood_manager` veritabanına doğrulanmış dökümü yükleyin. Var olan canlı veritabanını silmek bu kılavuzun otomatik adımı değildir.
4. `master.key` dosyasını `/var/lib/hoodmin/master.key` olarak `hoodmin:hoodmin`, 0600 ile geri yükleyin. `/var/lib/hoodmin` 0700 olmalıdır.
5. `configuration/` ve ortam ayarını `/etc/hoodmin` altında geri yükleyin; `hoodmin.env` için `root:hoodmin`, 0640 kullanın. Genel HTTPS adresini kontrol edin.
6. AutoSign `acme/` ağacını aynı konuma `/var/lib/hoodmin/acme/` geri koyun; `hoodmin:hoodmin` sahipliğini ve özel dosya/dizin izinlerini koruyun. Certbot hesap/özel anahtarlarını herkese okunur yapmayın. Varsa `/etc/letsencrypt` ağacını da özel izinleriyle geri yükleyin.
7. Yeni pakete yükseltiyorsanız önce yedek sürümle doğrulayın, ardından desteklenen geçişleri `hoodmin-db-setup` ile uygulayın. HTTPS/Nginx ve `/etc/hoodmin/configured` başlangıç işaretini kontrol edin.
8. Hizmeti başlatıp HTTPS girişini, VPN sunucusu bağlantılarını, yapılandırma onayını ve yeni anahtar üretimini sınayın. Sertifika ve yayıncı işleri için kayıtlı görevlerin durumunu kontrol edin.

Kaybolan panel parolasını ilk sahip JSON dosyasıyla sıfırlamış olmazsınız; dosya yalnız ilk parolayı kaydeder. Parola değiştirilmişse güncel parola kullanılmalıdır. Kurtarma kopyalarında parolaları ve özel anahtarları genel raporlara yazmayın.
