# Çalışma durumu — 19 Ekim 2026

Kullanıcı 17 Ekim duraklamasından sonra iki yeni SSH makinesi vererek devam edilmesini istedi. Yeni ortam üretim hedefiyle hazırlanmıştır; tam Console eşdeğerliği ve uzun süreli üretim onayı iddia edilmez.

## Son tamamlanan işler

- Önceki grup adres yenilemesi ve ortak PFX deposu korunur.
- Yeni `hoodmin` ve `vh01` kapsayıcıları, geçerli HTTPS, yeni sahip hesabı ve ilk gerçek VPN sunucusu çalışır.
- Debian paketi; kurulum/yükseltme, geçiş özeti denetimi ve korumalı systemd hizmeti.
- Gerçek ACME alma/yenileme köprüsü, PFX dağıtımı ve CA/TLS denetimi; Certbot zamanlayıcısı etkin.
- Şifreli günlük yedek ve diğer makineye sınırlı SSH kopyası; ayrı veritabanında ana anahtarla kurtarma doğrulandı.
- PFX değişiminde DNS çözümlemesi düzeltildi; 22 Rust testi ve ilgili gerçek sunucu sınamaları geçti.
- Genel sunucuda TCP/UDP/QUIC paket kanallarından UDP DNS ve sunucu TUN/NAT geçişi doğrulandı.

[Üretim ortamı ve erişim](uretim-ortami.md), [kurulum](uretim-kurulumu.md), [kanıtlar](dogrulama.md), [uyumluluk sınırları](protokol-uyumlulugu.md).

## UID/GID ve sahip parolası

Kullanıcı UID/GID taşımasını onayladı, ancak gerekliliğini sordu ve durdurulmuş `lxc-copy` yedeğini önerdi. Değerlendirme sonunda taşıma zorunlu bulunmadı; mevcut 100000 tabanlı eşlemenin korunacağı kullanıcıya açıklandı. Bu işlem uygulanmadı ve bekleyen onay olarak değerlendirilmez. Önemli kapsayıcı değişikliklerinden önce durdurulmuş bağımsız kopya yaklaşımı kullanılacak; bu turda üretim kapsayıcılarında bakım yapılmadı.

İlk sahip parolasını yerel dosyaya kopyalama adımı daha önce reddedildi. Parola üretim kapsayıcısında kalır; bu aktarım için ayrıca açık izin olmadan başka yol denenmemelidir. API ve sunucu içindeki mevcut sahip oturumu işlev kontrollerinde kullanılabildi; tarayıcıda güvenilir giriş sayfası doğrulandı.

## POST/PUT sınaması — onaylı gözlem tamamlandı

Kullanıcı geçici alıcı, şifreli deneme çiftliği adresini gönderme ve yalnız bu işin kaydını temizleme kapsamını açıkça onayladı. Eski `yapayzeka` QA kullanıldı; yeni üretim değiştirilmedi. [Ayrıntılı sınama kaydı](console-harici-yayim-sinamasi.md).

Kullanıcının paylaştığı VPS günlüğü ilk 400'ün ModSecurity JSON çözümleme hatası olduğunu gösterdi. Console'da içerik türünü başlıkla değiştirme denemesi mükerrer Content-Type hatası verdi. Body=`"{farm_token}"` ile geçerli JSON gönderilince alıcıya ulaşıldı; güvenlik duvarı değiştirilmedi. PUT & POST kipinde PUT=405 → POST=201 ve PUT=201 → POST gönderilmemesi doğrulandı. Alıcı JSON yüklemesini düz metin olarak sunduğunda Console durumu OK oldu. Başlık `{access_token}` ve gövde `{farm_token}` yer tutucuları doğrulandı; URL sorgusundaki `{access_token}` ise değiştirilmeden geldi. Form-Data denemesi 400 aldı; kodlaması henüz teyitli değil. Hoodmin harici yayıncısı henüz uygulanmadı.

Temizlik tamamlandı: Console listesi yeniden boş; geçici alıcı/veriler/Nginx yolu kaldırıldı; önceki Nginx dosyası aynen geri yüklendi. Son durumda geçici port kapalı, yol 404, QA giriş sayfası 200 ve hizmetler etkin. Yeni paket kurulmadı. Gizli içermeyen kanıt eski QA'da ve yerelde Git dışı `work/console-publication-qa-20261019-success.json` içinde korunur.

Kullanıcı, ModSecurity nedeniyle standart veri biçiminin değiştirilmemesini istedi. JSON sarmalaması yalnız teşhis sınamasıdır; Hoodmin için varsayılan olmayacak. `vhm-test.oktayaktogan.com.tr` üzerinde ModSecurity kapatıldığı bilgisi bekleniyor; geldiğinde ham gövde ve Form-Data yeniden sınanacak. Bu aşamada yeni geçici alıcı açılmadı.

## Sonraki özellikler

1. Harici POST/PUT şifreli adres yayımı.
2. Grup düzeyinde sunucu profili atama ve otomatik değişiklik yayma; açık seçili toplu uygulama zaten çalışır.
3. Kiracının panelden kendi ACME işini oluşturduğu AutoSign akışı. Mevcut root/Certbot entegrasyonu gerçek sunucuda çalışır.
4. Ana anahtar döndürme, harici hata bildirimi, yedek saklama/disk politikası ve uzun süreli yük/kesinti sınamaları.
5. Masaüstü işletim sistemi VPN rotalarının tümü, Windows kurulum çalıştırması ve kapalı kaynak davranışı gözlenmeyen ticari özellikler ayrıca değerlendirilir.

Eski `yapayzeka`/`vpnhood` ortamları gerileme sınaması için korunur. Kullanıcının eski `vpnhood-test` kaydı değiştirilmedi. Kaynak kardeş `VpnHood-Current` salt okunur kalmıştır. Özel sınama verileri, kimlikler ve kurtarma anahtarları `work/` içinde Git dışındadır; yayımlanmamalıdır.
