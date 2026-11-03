# Hoodmin elkitabı

Hoodmin, VpnHood VPN ortamının yönetim panelidir. Genel amaçlı sunucu, sanal makine veya kapsayıcı yönetimi sağlamaz. VPN sunucusunu resmî kurucuyla kurar; VPN ayarlarını, kullanıcı erişimini ve sertifikaları yönetir.

## İçindekiler

1. [Kurulum ve ilk giriş](#kurulum-ve-ilk-giriş)
2. [Menüler ve kavramlar](#menüler-ve-kavramlar)
3. [İlk VPN sunucusunu bağlama](#ilk-vpn-sunucusunu-bağlama)
4. [VPN erişimi verme](#vpn-erişimi-verme)
5. [Profiller ve gruplar](#profiller-ve-gruplar)
6. [Sertifikalar ve adres yenileme](#sertifikalar-ve-adres-yenileme)
7. [Ekip ve otomasyon](#ekip-ve-otomasyon)
8. [Günlük işletim](#günlük-işletim)

## Kurulum ve ilk giriş

1. [Debian kurulum kılavuzunu](uretim-kurulumu.md) izleyin. Panel için bir HTTPS alan adı, VPN sunucusu için ayrı bir bağlantı adresi kullanın.
2. İlk sahip hesabının e-postası ve parolası panelin kurulduğu makinede `/root/hoodmin-first-owner.json` dosyasındadır. Bu dosyayı yalnız güvenilir root oturumunda okuyun. Tarayıcıdan indirme bağlantısı veya ortak varsayılan parola yoktur.
3. Panelin HTTPS adresinden giriş yapın. **Hesabım** bölümünde parolanızı değiştirin. Parola değişikliği tüm açık panel oturumlarınızı kapatır; VPN anahtarlarınızı değiştirmez.
4. Üst çubuktaki dil seçiciden Türkçe, English veya العربية seçin. Güneş/ay düğmesi açık ve koyu temayı değiştirir. Tercihler tarayıcıda saklanır; Arapçada yerleşim sağdan sola döner.
5. Yöneticiyseniz **Hesabım → Çalışma alanı** alanından kuruluş adını düzenleyin.

Panel hesapları yönetim içindir. VPN kullanıcısının bağlanması için panel hesabı açmanız gerekmez; ona bir VPN erişim anahtarı verirsiniz.

## Menüler ve kavramlar

| Bölüm | Kullanım |
|---|---|
| Genel bakış | VPN sunucularının durumu, oturum ve trafik özeti; sık kullanılan işlemlere erişim. |
| Sunucular | VPN sunucusu ekleme/düzenleme, kurulum, dinleyici/ağ ayarları, gruplar ve sertifikalar. |
| Erişim anahtarları | Tekli/toplu `vh://` anahtar, hız/cihaz/kota/süre sınırları ve erişim profilleri. |
| VPN oturumları | Bağlı VPN oturumlarını inceleme ve yetkiniz varsa sonlandırma. |
| Raporlar | Zaman/sunucu filtreli CPU, bellek, oturum, hız ve bildirilen ülkeye göre cihaz sayıları. |
| Denetim kaydı | Yönetim işlemlerinin kaydı; uygulama bu kayıtları değiştiremez/silemez. |
| Panel kullanıcıları | İnsan üyelerin rolleri ve erişimi; botlar ve API anahtarları. |
| Hesabım | Panel parolası ve yetkiniz varsa çalışma alanı adı. |

**Kiracı (tenant)** bir kuruluşun çalışma alanıdır. Sunucu, grup, profil ve anahtarlar bu alana aittir. Başka kiracının sunucusuna erişim verilmez; kiracılar arasında ortak VPN havuzu yoktur.

**Sunucu profili**, VPN dinleyici/ağ/oturum ve kapasite ayarlarını içerir. **Erişim profili**, kullanıcı anahtarının hız, kota, süre ve cihaz sınırlarını belirler. Bunlar farklı amaçlara hizmet eder.

Soru işareti düğmeleri ilgili alanın açıklamasını açar. Escape ile kapatabilirsiniz. Bir yapılandırmayı kaydetmek veritabanına kayıttır; VPN sunucusunun uygulama onayını da kontrol edin.

## İlk VPN sunucusunu bağlama

1. VPN makinesinin dış IP adresini veya FQDN'ini ve kullanılacak TCP portunu belirleyin. İlgili güvenlik duvarı/NAT yolunu açın. UDP ve QUIC kullanacaksanız kendi dinleyici portlarını da yönlendirin. Panelin HTTPS portuyla aynı makinede aynı IP/port üzerinde iki hizmet dinleyemez.
2. **Sunucular → Sunucu ekle** bölümünde adı, **VPN adresi (DNS veya IP)**, TCP portu ve isteğe bağlı grubu girin. Buradaki adres panelin değil, VPN istemcisinin bağlanacağı adrestir.
3. Kaydettiğinizde bağlantı ayarı ve kurulum seçenekleri gösterilir. İçerik sunucuya özel sırlar taşır; herkese açık paylaşmayın.
4. Bir kurulum yöntemi seçin:
   - **Linux: elle komut:** oluşturulan komutu VPN makinesinde çalıştırın. Resmî hazır VpnHood kurucusunu indirir.
   - **SSH:** hedef adres/port/kullanıcı ve doğrulanmış SHA-256 SSH sunucu parmak izini girin. Parola veya özel anahtar kullanın; gerekiyorsa sudo parolası verin. Parmak izini hedefteki güvenilir konsoldan veya yöneticinizden alın; eşleşmediğinde kurulumu zorlamayın.
   - **Windows:** oluşturulan PowerShell komutunu yetkili oturumda çalıştırın. Windows kurulumu ayrı işletim sistemi doğrulaması gerektirir.
   - **Mevcut kurulum:** oluşturulan JSON'u sunucunun `storage/appsettings.json` ayarlarıyla birleştirin; gerekli sunucu hizmeti yeniden başlatmasını yapın.
5. SSH kullanıyorsanız iş durumunu ve temizlenmiş günlüğünü izleyin. Kesilen bir kurulum işi kendiliğinden tekrar çalıştırılmaz; hedefin durumunu inceleyip yeniden başlatın.
6. Sunucu panelin `/api/agent/` yoluna HTTPS üzerinden bağlandığında durum ekranını kontrol edin. Çevrimiçi toleransı en az 90 saniye veya bildirim aralığının üç katıdır; bu gösterge gerçek istemci bağlantı sınamasının yerini almaz.
7. Sunucuyu düzenleyerek IP/FQDN, duyurulan TCP portu, ad, grup ve bakım durumunu değiştirebilirsiniz. Duyurulan portu değiştirmek dinleyici ve NAT kuralını kendiliğinden değiştirmez. Önceden verilmiş anahtarları adres yenileme veya yeniden paylaşma ile güncelleyin.

**Bağlantı kimliğini yenile**, sunucunun panelle konuştuğu kimliği hemen geçersizleştirir. Onay ekranını okuyun; yeni ayarı VPN sunucusuna uygulayana kadar haberleşme durur. Bu işlem VPN erişim anahtarını değiştirmez.

## VPN erişimi verme

1. **Erişim anahtarları** bölümünde sunucuyu seçin.
2. Ad/önek ve adet girin; bir işlemde 1–100 anahtar oluşturulur.
3. Bir erişim profili seçin veya formdaki cihaz, trafik ve süre sınırlarını belirleyin. Trafik kotası GiB'dir; 0 sınırsızdır. Son tarih boşsa kesin bitiş tarihi yoktur. Zamanı saat dilimi içeren ISO 8601 biçiminde yazın.
4. Oluşturulan `vh://` anahtarını yetkili VPN kullanıcısına güvenli yoldan iletin; resmî VpnHood istemcisine aktarın.
5. İlk bağlantıda **VPN oturumları** ve kullanım sayaçlarını kontrol edin.
6. Gerektiğinde anahtar politikasını düzenleyin veya seçili anahtarları topluca iptal edin, etkinleştirin, kota/süre değiştirin. Onay metnini okuyun; değişiklik etkin bağlantıları etkileyebilir.

Cihaz sınırı aynı anahtarı eşzamanlı kullanabilecek farklı cihazları sınırlar; panel üyesi sayısını sınırlamaz. Profilde toplam kota, dönemlik kota, ilk kullanımdan başlayan ömür ve kesin son tarih ayrı kurallardır. Dönem süresini açıkça seçin; aylık takvim sıfırlaması varsayılmaz.

## Profiller ve gruplar

1. **Sunucu profilleri** bölümünde ortak VPN ayarlarını oluşturun. Gelişmiş alanlar kaynak sözleşmesinin türlerini kullanır; bilinmeyen alanlar reddedilir.
2. İsterseniz seçili sunuculara tek seferlik uygulayın. Sürekli yönetim için bir gruba atayın.
3. **Sunucular → grup → Grup sunucu profili** bölümünde profili seçip etkiyi onaylayın. Mevcut, yeni ve gruba taşınan sunucular profili alır. Bağlı profil kaydı bütün ilgili grupları etkiler.
4. VPN sunucularının uygulama sürümünü ve dinleyici durumunu kontrol edin. Çevrimdışı üye ayarı yeniden bağlandığında alır.
5. **Erişim profilleri** bölümünde anahtar politikalarını oluşturun. Grup erişimi (`farmAccess`) açıksa yalnız aynı kiracı ve grubun uygun sunucuları arasında yönlendirme yapılır; konum, kapasite ve göreli güç seçimde kullanılır.

Grup profilinden ayrılmak son ayarları geri almaz. Tekil ayar değişikliği yapılabilir; sonraki bağlı profil kaydı onu ezebilir. [Ayrıntılı davranış](grup-sunucu-profili.md).

## Sertifikalar ve adres yenileme

- **Sunucu → Sertifikalar:** özel anahtarlı PFX dosyasını parolasıyla yükleyin. DNS adı, geçerlilik ve anahtar eşleşmesi denetlenir. Varsayılan sertifika değişimini onaylayın ve sunucunun uygulama durumunu izleyin.
- **Otomatik sertifika (AutoSign):** [adım adım kılavuzu](autosign.md) izleyin. VPN alan adı ve dış TCP 80 doğrulama yolu gereklidir. Panelin HTTPS sertifikası ayrı yönetilir.
- **Grup sertifikası:** ortak PFX mevcut/yeni/taşınan üyelere dağıtılır. Üyelikten çıkmak daha önce dağıtılmış özel anahtarı geri çekmez.
- **Grup adresi yayımı:** ilk sunucuya ulaşılamadığında uygun canlı üyenin adresinin yenilenmesini sağlar. Yeni grup erişimli anahtarlar yenileme URL'sini içerir; eski anahtara URL sonradan eklenmez.
- **Harici adres yayımı:** dosya/CDN için HTTPS PUT/POST veya form yüklemesi yapılandırılır. Yükleme ve okuma adresleri ayrıdır. [Hedef oluşturma ve sır yönetimi](harici-adres-yayimi.md).

Sertifika/adres değişince eski anahtarların parmak izini ve adresini güncelleyin. Yayım kullanmıyorsanız anahtarı yeniden paylaşın. Bir sunucu kimliği veya VPN sırrı, diğer kiracılarla ortaklaştırılmaz.

## Ekip ve otomasyon

| Rol | Görev |
|---|---|
| Sahip (owner) | Yönetici işlemleri ve sahip atama; son etkin sahip korunur. |
| Yönetici (admin) | Ekip, bot, çalışma alanı ve VPN yönetimi; AutoSign ayarları. |
| Operatör (operator) | VPN sunucusu, grup, profil, anahtar, sertifika ve oturum işlemleri; ekip/bot yönetemez, AutoSign ayarlarını değiştiremez. |
| Erişim anahtarı yazarı (token_writer) | VPN anahtarları/erişim profilleri ve oturum kapatma; sunucu ayarını değiştiremez. |
| Salt okunur (viewer) | Liste ve durum; erişim sırlarını indiremez, değişiklik yapamaz. |

**Panel kullanıcıları** bölümünden insan üye ekleyin; e-posta davet hizmeti yoktur, hesabı oluşturup ilk bilgileri güvenli iletin. **Botlar ve API anahtarları** bölümünde otomasyon için uygun rol ve 1–365 günlük süre seçin. Bot anahtarı yalnız oluşturulurken gösterilir. Süre bitmeden yenisini oluşturun ve eskisini iptal edin. Bot, parola ile giriş yapamaz veya sahip rolü alamaz. [JSON API başvurusu](yonetim-api.md).

Ödüllü reklam doğrulaması yalnız güvenilir sunucunun kimlikli bildirimiyle çalışır; istemci bildirimi tek başına ödül vermez. Hazır reklam ağı veya ödeme sistemi entegrasyonu değildir.

## Günlük işletim

1. **Genel bakış** ve **Raporlar** ile çevrimdışı üyeleri, kapasiteyi ve kullanım eğilimini izleyin. Ülke bilgisi bildirilen değerdir; bağımsız IP konumlandırması yapılmaz.
2. **Denetim kaydı** ile yönetim değişikliklerini inceleyin. İzleme günlüğünü erişim anahtarı, parola veya PFX paylaşma kanalı olarak kullanmayın.
3. [Yedekleme](yedekleme.md) düzenini kurun; ayrı makinedeki şifreli kopyayı ve kurtarma anahtarını düzenli doğrulayın. Saklama süresini ve disk kullanımını ayrıca yönetin.
4. [Kurulum kılavuzundaki yükseltme adımlarını](uretim-kurulumu.md#yükseltme) uygulayın. Ana anahtarı ve ortam dosyasını koruyun; VPN sürümünü kontrollü güncelleyin.
5. Sertifika durumunu ve AutoSign hatalarını kontrol edin. E-posta alanı, harici alarm hizmeti kurulduğu anlamına gelmez.
6. Hata durumunda [sorun giderme kılavuzunu](sorun-giderme.md) kullanın.

## Bildirimler ve işletim

**Bildirimler** menüsünde webhook/SMTPS kanalı oluşturun, sınayıp son gönderimleri kontrol edin. Anahtar döndürme ve yedek saklama panel makinesinde root yetkili `hoodmin-cli` ile yönetilir. [Adım adım işletim politikaları](isletim-politikalari.md).

Arayüz tarayıcıda çalışan statik uygulamadır (SPA); JavaScript gerekir. API bağlantısı kesilirse basit hata gösterilir ve tekrar denenir. Dil/tema tercihi tarayıcıda saklanır; oturum sırrı HttpOnly çerezindedir.

Arayüz kök adreste açılır (`/`); sunucular `/servers`, VPN anahtarları `/access-keys`, hesabınız `/account` altındadır. [Web sunum kılavuzu](web-sunumu.md), aynı veya ayrı Nginx makinesindeki statik dosya kurulumunu anlatır.
