# Sorun giderme

Komutlar panel makinesinde root oturumu içindir. Kapsayıcı kullanıyorsanız önce panel kapsayıcısına girin.

## Panel açılmıyor

```bash
hoodmin-cli service status
systemctl status nginx --no-pager
journalctl -u hoodmin.service -n 100 --no-pager
nginx -t
curl --fail --show-error http://127.0.0.1:3080/healthz
curl --fail --show-error http://127.0.0.1:3080/readyz
```

- HTTP hizmeti çalışıp `/readyz` 503 veriyorsa veritabanı bağlantısı, TLS, şema ve ana anahtarı denetleyin. Hizmet bunlar olmadan da ayakta kalır; ayrıntı günlüktedir.
- Hizmet başlamıyorsa yapılandırma JSON dosyası, veritabanı geçişleri ve ana anahtarın sahiplik/izinlerini kontrol edin.
- İç sağlık yolu çalışıp genel HTTPS çalışmıyorsa DNS, sertifika, Nginx ve NAT yolunu kontrol edin.
- `502/504` için ters vekil hata günlüğüne bakın. Çok katmanlı vekilde her bağlantıyı kendi sertifika adıyla doğrulayın; TLS doğrulamasını kapatmayın.
- Formlar 403 alıyorsa `public_origin` ile tarayıcının adresi (şema, ad ve port) eşleşmelidir. Alan adı değişince yeniden giriş yapın.

## İlk erişim bilgileri nerede?

Kurulum sırasında verdiğiniz e-posta ve rastgele parola `/root/hoodmin-first-owner.json` içindedir. Dosya **panelin kurulduğu makinede/kapsayıcıdadır**. `hoodmin` panel kullanıcı adı değildir. Parola ilk girişten sonra değiştirildiyse dosyadaki ilk parola artık geçerli değildir. Mevcut hesapta `hoodmin-cli owner-init` çalıştırmak parolayı sıfırlamaz.

## VPN sunucusu çevrimdışı

1. VPN makinesinden panel HTTPS adresinin DNS/TLS erişimini kontrol edin.
2. Sunucudaki `storage/appsettings.json` içindeki yönetim kök adresi ve kimliği kontrol edin; sırları günlük/sohbet çıktısına taşımayın.
3. Sunucu hizmeti durumuna ve günlüğüne bakın; resmî kurucunun varsayılan hizmeti `VpnHoodServer.service` olur.
4. Bağlantı kimliği yenilendiyse yeni ayarı sunucuya uygulayın. Panelde bir düğmeye basmak hedef dosyayı kendiliğinden güncellemez.
5. Yerel `ServerConfig` ayarı merkezi ayarları geçersiz kılabilir. Sunucunun uygulanan yapılandırma sürümünü ve dinleyici durumunu inceleyin.

Çevrimiçi bilgisi son bildirime dayanır; VPN portunun internete açık olduğunu tek başına kanıtlamaz.

## VPN istemcisi bağlanamıyor

- Anahtarda ilan edilen IP/FQDN ve port ile gerçek dinleyici/NAT/güvenlik duvarını karşılaştırın. TCP/UDP/QUIC ayrı port/protokol kurallarıdır.
- Kota, bitiş tarihi, cihaz sınırı, bakım durumu ve grup kapasitesini inceleyin.
- Adres veya sertifika değiştiyse adres yayımıyla yenileyin ya da yeni anahtarı yeniden paylaşın.
- Özel ağ hedefi engelleniyorsa ağ filtrelerini kontrol edin. LXC'de TUN, NAT ve çekirdek izinleri VPN veri yolunu ayrıca etkiler.

## AutoSign başarısız

1. Alan adının A ve varsa AAAA kaydını kontrol edin. Dış TCP 80 panel yerine **VPN sunucusuna** gitmelidir.
2. Sunucu çevrimiçi olmalı ve merkezi HTTP-01 ayarını alabilmelidir. Başka süreç aynı portu kullanmamalıdır.
3. AutoSign ekranında test/gerçek CA seçimini, son hatayı ve sonraki kontrolü inceleyin. Test sertifikası VPN'e uygulanmaz.
4. Ayrıntılı Certbot günlüğü `/var/lib/hoodmin/acme/` içindeki görev çalışma ağacındadır; özel anahtar/hesap dosyalarını paylaşmayın.
5. Çalışan işte düzenleme yapılamaz. Elle tekrar için beş dakika bekleyin. Normal kontrol 12 saattir; her kontrolde yeni sertifika istenmez.
6. Eşzamanlı HTTP-01 ayar değişimi saptanırsa temizlik üzerine yazmaz; iş durumunu ve dinleyici bayrağını inceleyin. Aynı sertifikayı ikinci Certbot zamanlayıcısıyla yönetmeyin.

## Harici yayım başarısız

Yükleme adresi ile istemcinin GET okuma adresini ayrı kontrol edin. Hedefin istediği içerik türü/şablonu kullanın; ham şifreli metin JSON değildir. WAF günlüğünde gövde çözümleme hatası varsa HTTP isteğinin gerçek içerik türünü düzeltin; otomatik JSON sarmalama varsaymayın. CDN önbelleği yeni içeriği geciktirebilir. Özel/yerel ağ hedefleri güvenlik gereği reddedilir. [Yayım başvurusu](harici-adres-yayimi.md).

## Eski form veya izin hatası

`409` kaynak sürümünün değiştiğini veya işin çalıştığını gösterebilir; sayfayı yenileyip durumu kontrol edin. `403` yetki/Origin/CSRF, `404` kiracıya ait olmayan veya bulunmayan kaynak anlamına gelebilir. Bot için `401` geçersiz, süresi dolmuş veya iptal edilmiş anahtardır. Bot rolü insan hesabından bağımsızdır.
