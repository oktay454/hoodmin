# Panelden AutoSign

Sunucu → Sertifikalar → Otomatik sertifika (AutoSign). Yönetici/sahip alan adı, CA iletişim e-postası, test/gerçek CA ve etkinlik seçer. Alan adı/e-postanın Let's Encrypt'e gönderilmesi, CA koşulları ve VPN sertifikasına etkisi açıkça onaylanır. HTTP-01 için genel DNS ve dış TCP80 VPN sunucusuna ulaşmalıdır. Joker alan adı bu doğrulama yöntemiyle desteklenmez.

Test CA sertifikası güvenilir değildir ve VPN sunucusuna uygulanmaz. Gerçek CA sertifikası varsayılan PFX olarak uygulanır; sunucunun configCode onayı beklenir. Eski erişim anahtarlarının parmak izi adres yayımı veya yeniden paylaşım yoluyla güncellenmelidir.

İşler veritabanında kalıcıdır. İşçi beş saniyede bir hazır işleri bulur, 20 dakikalık kirayla alır; kesilen işler süre dolunca toparlanır. Certbot denemesi en fazla 12 dakika sürer. Başarı veya hatadan sonra 12 saatte bir kontrol edilir; Certbot yalnız sertifika yenileme zamanı geldiğinde yeniden talep gönderir. Elle tekrar en erken beş dakika sonra mümkündür. Çalışan iş düzenlenemez. Kapatmak mevcut sertifikayı veya özel dosyaları silmez.

Certbot root çalışmaz. Yapılandırma/hesap anahtarı, günlük ve sertifikalar ana anahtarın yanındaki acme/kiracı/görev/ayar-özeti dizinlerinde özel izinlerle saklanır. CA hesap anahtarı Certbot'un standart özel dosyasıdır; veritabanına veya API yanıtına konmaz. Alan adı/e-posta/CA değişince ayrı çalışma dizini kullanılır. Önceki özel dosyalar korunur; yedekleme kapsamına alınmalıdır. hoodmin-backup bu acme ağacını da özel yedeğe kopyalar ve dosya özetlerini kaydeder; kurtarmada aynı konuma vhmgr sahipliği ve özel izinleriyle geri yükleyin.

HTTP-01 kancası yalnız kiracının kiralanmış görevindeki sunucuyu yönetir; bot API anahtarı veya SSH/root yetkisi kullanılmaz. Önceki HTTP-01 durumu saklanır, işlem sonunda geri yüklenir ve yalnız görevin token kaydı silinir. Eşzamanlı kullanıcı değişikliği saptanırsa üzerine yazılmaz ve hata bildirilir; işletmen HTTP-01 durumunu denetlemelidir.

GET/POST /api/manage/servers/{id}/autosign aynı rol/onay/sürüm kurallarını kullanır. POST alanları: version (ilk oluşturmada yok), domain, email, staging, enabled, confirmed, agree_tos. UI'da durum, son hata, son deneme/başarı, sertifika bitişi ve sonraki kontrol görünür. CA hata ayrıntısı özel Certbot günlüğündedir; panel özel anahtar veya challenge proof döndürmez.

Aynı sertifikayı eski root/Certbot zamanlayıcısı ve AutoSign ile birlikte yönetmeyin. Geçişte eski hedef yenileme yapılandırmasını kapatın; panelin HTTPS sertifikası ayrı kalır. Bu sürüm her sunucu için bir alan adı yönetir; grup sertifikası deposuna toplu AutoSign ayrı akış olarak sunulmaz.

Kaynak: [Certbot manual hooks](https://eff-certbot.readthedocs.io/en/stable/using.html#pre-and-post-validation-hooks).
