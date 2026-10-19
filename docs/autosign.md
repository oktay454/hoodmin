# VPN sertifikasını AutoSign ile yönetme

AutoSign, VPN sunucusunun Let's Encrypt sertifikasını alır ve yeniler. Panelin Nginx HTTPS sertifikasını değiştirmez. Bu ekranı sahip veya yönetici yapılandırabilir.

## Ön koşullar

- VPN sunucusu Hoodmin'e bağlı ve çevrimiçi olmalıdır.
- Kullanılacak FQDN'in DNS kayıtları VPN sunucusuna gitmelidir; IP adresi veya joker alan adı kabul edilmez.
- İnternetten TCP 80, VPN sunucusunun HTTP-01 dinleyicisine ulaşmalıdır. NAT/güvenlik duvarı ve varsa IPv6 yolunu kontrol edin.
- Aynı VPN sertifikasını başka bir Certbot zamanlayıcısı yönetmemelidir. Panelin kendi HTTPS yenilemesi ayrı kalır.

## Adım adım kullanım

1. **Sunucular → sunucu → Sertifikalar → Otomatik sertifika (AutoSign)** bölümünü açın.
2. VPN alan adını ve sertifika otoritesi iletişim e-postasını girin.
3. Önce **Test CA** seçeneğiyle doğrulamayı deneyin. Alan adı/e-postanın Let's Encrypt'e gönderilmesini, hizmet koşullarını ve işlemin etkisini okuyup onaylayın.
4. Etkinleştirip **Kaydet ve kontrol et** düğmesine basın. Durum ve son hata alanlarını izleyin.
5. Test başarılı olunca test seçimini kaldırarak gerçek CA için aynı onaylarla kaydedin. Test sertifikası güvenilir değildir ve VPN sunucusuna uygulanmaz.
6. Gerçek işlem tamamlanınca sertifika bitişini ve sunucunun yapılandırma onayını kontrol edin. Geçerli PFX varsayılan VPN sertifikası olur.
7. Önceden verilmiş VPN anahtarlarının parmak izini adres yayımıyla yenileyin; yayım kullanılmıyorsa anahtarları yeniden paylaşın.

## Yenileme ve durum

Durum ekranında son deneme/başarı, sertifika bitişi ve sonraki kontrol UTC olarak gösterilir. Kontrol 12 saatte bir yapılır; Certbot yalnız yenileme gerektiğinde yeni sertifika ister. Değişmeyen sertifika yeniden uygulanmaz. Hatalı işler de 12 saat sonra tekrar kontrol edilir; elle tekrar en erken beş dakika sonra yapılabilir.

Çalışan iş düzenlenemez. Devre dışı bırakmak sonraki kontrolleri durdurur; son sertifikayı, CA hesabını veya özel dosyaları silmez. Görevler veritabanında kalıcıdır. Yarıda kalan işin 20 dakikalık kirası dolunca yeniden alınması mümkündür; tek Certbot denemesi en fazla 12 dakika sürer.

HTTP-01 doğrulamasında önceki dinleyici durumu saklanır; işlem sonunda geri yüklenir ve yalnız o işe ait geçici token kaldırılır. Eşzamanlı kullanıcı değişikliği saptanırsa üzerine yazılmaz; hata durumunda dinleyici ayarını inceleyin.

## Özel dosyalar ve sınırlar

Certbot ayrıcalıksız `hoodmin` hesabında çalışır. CA hesapları, günlükler, sertifikalar ve özel anahtarlar `/var/lib/hoodmin/acme/` altında özel izinlerle saklanır. Bunlar standart özel Certbot dosyalarıdır; bütün ACME ağacı ayrıca AES-GCM ile şifrelenmez. Dosyalar API/HTML yanıtına konmaz. `hoodmin-cli backup create` bu ağacı içerir; kurtarmada aynı yol/sahiplik/izinler korunmalıdır. [Yedekleme](yedekleme.md).

Alan adı, e-posta veya CA değişince ayrı çalışma ağacı kullanılır; eski dosyalar kendiliğinden silinmez. Her VPN sunucusu için tek FQDN ve HTTP-01 desteklenir. DNS-01, joker alan adı ve grup deposuna toplu AutoSign sunulmaz. E-posta kaydı hata bildirimi hizmeti değildir; iş durumunu ve günlükleri izleyin.

## API

`GET/POST /api/manage/servers/{id}/autosign` aynı kiracı/rol/onay kurallarını uygular. POST alanları: `version` (ilk oluşturmada yok), `domain`, `email`, `staging`, `enabled`, `confirmed`, `agree_tos`. Yanıt hesap anahtarı veya doğrulama sırrı içermez. [Yönetim API'si](yonetim-api.md).
