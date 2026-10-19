# Yedekleme ve kurtarma

Veritabanı, ana anahtar, yapılandırma ve sertifika dosyalarını birlikte koruyun. Ana anahtar kaybolursa şifreli sunucu ayarları ve yönetim sırları kullanılamaz. Yeni anahtar oluşturmak kurtarma değildir.

## 1. Araçlar ve bağlantı

Yedek komutları Rust `hoodmin-cli` içindedir. Veritabanı dökümü için sunucu sürümüne uygun Debian `postgresql-client` (`pg_dump`), şifreleme için ayrıca `age` gerekir. Bunlar Hoodmin paketinin zorunlu bağımlılıkları değildir; kullanacağınız özelliğe göre ayrıca kurun.

Yerel soket bağlantısında `pg_dump` yerel `postgres` hesabıyla çalışır. Uzak PostgreSQL'de uygulama rolü RLS atlayamadığı için bütün kiracıların yönetici yedeğini alamaz; yedek yetkili bağlantıyı ayrı 0600 dosyadan verin. Bu bağlantı dizesi **uygulamanın veritabanını** hedeflemeli ve TCP'de `sslmode=verify-full` kullanmalıdır.

## 2. Yedek alma

Yerel veritabanı:

```bash
hoodmin-cli backup create
```

Uzak veritabanı:

```bash
hoodmin-cli backup create --admin-url-file /root/hoodmin-backup-db.url
```

Komut `/var/backups/hoodmin/` altında yeni 0700 dizin oluşturur. `--directory` ile başka hedef seçilebilir. İçerik:

| Dosya/dizin | İçerik |
|---|---|
| `database.dump` | PostgreSQL özel biçimli dökümü. |
| `master.key` | Şifreli yönetim verilerini açan ana anahtar. |
| `hoodmin.json`, `configuration/` | Yapılandırma ve yapılandırma dizininin kopyası; bağlantı sırları içerebilir. |
| `acme/` | Varsa AutoSign hesabı, sertifikalar ve özel anahtarlar. |
| `letsencrypt/` | Varsa panelin Certbot dosyaları. |
| `package-version.txt` | Paket sürümü. |
| `SHA256SUMS` | Temel dosyaların özetleri. |

Yapılandırma dizini dışındaki özel veritabanı TLS dosyalarını ayrıca koruyun; yollarını kurtarma planına ekleyin. Yedek yönetici bağlantı dosyası ve age özel kurtarma kimliği kendiliğinden arşive eklenmez.

Dosya/veritabanı tutarlılığı için çalışan işleri bitirip bakım penceresinde hizmeti durdurarak yedek alın veya kapsayıcının durdurulmuş kopyasını kullanın. Aynı diskteki yedek disk kaybına karşı korumaz. Başarısız döküm `.partial` olarak kalabilir; bunu başarılı yedek saymayın.

## 3. Şifreli dış kopya

Bağımsız güvenli makinede `age-keygen` ile alıcı/kurtarma kimliği üretin. Yalnız açık `age1…` alıcı değerini panelde `/etc/hoodmin/backup-recipient.txt` dosyasına yazın. Özel kimliği ayrı yerde saklayın.

```bash
hoodmin-cli backup export
```

Uzak veritabanında ayrıca `--admin-url-file /root/hoodmin-backup-db.url` verin. Komut yeni yedek oluşturup `/var/backups/hoodmin/latest.age` dosyasını atomik yeniler. Bu dosyayı bağımsız makineye kopyalayın. Yalnız sabit şifreli dosyayı okuyan komut ve kaynak IP ile sınırlandırılmış SSH anahtarı kullanabilirsiniz:

```bash
hoodmin-cli backup pull --host panel.example.com --user root --identity /root/.ssh/hoodmin-backup-pull --known-hosts /root/.ssh/hoodmin-backup-known-hosts
```

`/usr/share/doc/hoodmin/examples/` içindeki systemd örneklerini kendi adreslerinize/yedek bağlantınıza göre düzenleyin. Önce elle yedek oluşturarak hedef dizini hazırlayın. Zamanlayıcılar kendiliğinden etkinleşmez; eski yedekler otomatik silinmez. Saklama süresi, disk izlemesi ve temizliği ayrıca yönetin.

## 4. Doğrulama

Özel dizinde şifreli arşivi açın:

```bash
umask 077
install -d -m 0700 /root/hoodmin-recovery
age --decrypt --identity /GUVENLI_YOL/kurtarma.agekey --output /root/hoodmin-recovery/backup.tar /GUVENLI_YOL/latest.age
tar -tf /root/hoodmin-recovery/backup.tar
```

Kaynağı ve arşiv yollarını kontrol edin; sonra özel dizine açıp `sha256sum --check SHA256SUMS` çalıştırın. Özet kontrolü tam kurtarma sınamasının yerini almaz.

Canlı veritabanına yazmayın. Ayrı boş hedefte uygun PostgreSQL sürümü, yönetim rolü ve uygulama rolünü hazırlayın; `pg_restore --exit-on-error` ile dökümü yükleyin. Örnek yerel hedef:

```bash
runuser -u postgres -- createdb hoodmin_restore_check
runuser -u postgres -- pg_restore --exit-on-error --dbname=hoodmin_restore_check < /OZEL_YEDEK_DIZINI/database.dump
```

Uzak hedefte PostgreSQL istemcisinin bağlantı/TLS seçeneklerini kullanın; parola argümanına yazmayın. Hesap/sunucu sayıları, şema kayıtları, RLS ve yedekteki anahtarla verilerin açılması denetlenmelidir. Deneme panelini gerçek VPN/CA/yayım hedeflerine bağlamayın; kalıcı işler dış etki oluşturabilir.

## 5. Yerine geçecek kurulum

1. Özgün hizmeti durdurun. Aynı veriyi yöneten iki etkin panel bırakmayın.
2. Uygun paket ve PostgreSQL sürümüyle ayrı makine hazırlayın; ilk sahip oluşturmayın, hesaplar dökümden gelir.
3. Doğrulanmış dökümü boş hedefe yükleyin. Ana anahtarı `/var/lib/hoodmin/master.key` altında `hoodmin:hoodmin`, 0600 ile geri koyun.
4. `hoodmin.json` ve yapılandırma dizinini `/etc/hoodmin` altında geri yükleyin; JSON dosyası `root:hoodmin`, 0640 olmalıdır. Yeni veritabanı adresi/TLS/HTTPS adresini CLI ile ayarlayın; ana anahtarı değiştirmeyin.
5. `acme/` ağacını `/var/lib/hoodmin/acme/` altında hizmet hesabının sahipliği ve özel izinleriyle geri yükleyin. Varsa `/etc/letsencrypt` ve harici TLS dosyalarını özel izinleriyle koruyun.
6. Gerekiyorsa `hoodmin-cli database init` ile desteklenen geçişleri uygulayın. Hizmeti başlatıp `/readyz`, giriş, agent bağlantısı ve anahtar üretimini doğrulayın.

İlk sahip JSON dosyası yalnız ilk parolayı kaydeder; güncel parolayı sıfırlamaz. Sırlar hiçbir genel rapora eklenmemelidir.
