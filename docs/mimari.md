# Hoodmin mimarisi

Hoodmin, VpnHood VPN ortamını yöneten modüler bir Rust uygulamasıdır. Genel sunucu/kapsayıcı yönetimi sağlamaz. Her kuruluş kendi sunucularını getirir; bir VPN sunucusu tek kiracıya aittir. Başka VPN motorları için uygulanmış adaptör yoktur; mevcut API ve ayarlar VpnHood sözleşmesini izler.

## Bileşenler ve veri yolu

- Rust/Tokio/Axum: HTTPS vekilinin arkasındaki uygulama, yönetim ve agent API'leri.
- Leptos sunucuda sayfa üretimi (SSR): standart HTML formları; küçük JavaScript ile tema, yardım, kopyalama, toplu seçim ve canlı sayaçlar. Tarayıcıda WASM/hidrasyon kullanılmaz.
- PostgreSQL/SQLx: kiracı kaynakları, oturum kararları, denetim ve kalıcı AutoSign/yayım işleri.
- Nginx: panelin TLS sonlandırması; Rust `127.0.0.1:3080` üzerinde dinler.
- Resmî VpnHood sunucusu: panelin `/api/agent/` yoluna HTTPS isteği yapar. VPN trafiği istemci ile bu sunucu arasındadır; panelden geçmez.
- SSH yürütücüsü: yalnız resmî kurulum/yeniden kurulum işi. Agent protokolünde genel kabuk komutu yoktur.
- Certbot: sunucu başına HTTP-01 AutoSign; ayrıcalıksız hizmet hesabında çalışır.

## Kimlik ve yetki

İnsan hesapları Argon2id parola özeti kullanır. Rastgele panel oturum sırrının yalnız SHA-256 özeti saklanır. Çerezler Secure, HttpOnly, SameSite=Strict ve `__Host` kapsamındadır. Oturum sekiz saattir; parola değişimi açık oturumları kapatır. Rol her istekte yeniden denetlenir. Giriş denemeleri e-posta ve güvenilir vekilden gelen IP üzerinden sınırlanır.

Beş rol owner/admin/operator/token_writer/viewer vardır. Kiracı kaynakları oturumdan veya doğrulanmış agent/bot kimliğinden belirlenir; kullanıcı gövdesindeki kiracı kimliğine güvenilmez. Bot anahtarı rol ve süre taşır; yalnız özeti saklanır. Tarayıcı değişikliklerinde CSRF ve Origin doğrulanır. Son etkin sahip/yönetici eşzamanlı işlemlere karşı korunur.

## Kiracı ve sır yalıtımı

Kaynak tablolarında zorunlu satır düzeyi güvenlik (FORCE RLS) ve kiracı içeren yabancı anahtarlar vardır. İşlem kapsamındaki kiracı ayarı bağlantı havuzuna sızmaz. Giriş öncesi hesap/oturum kimlik tabloları küreseldir. `hoodmin` veritabanı rolü süper kullanıcı değildir ve RLS atlayamaz. Sistem hesabı/grubu da `hoodmin` adını kullanır.

Sunucu yapılandırmaları, PFX ve yayıncı sırları kaynak/kimlik bağlamına bağlı AES-256-GCM ile korunur. Ana anahtar ayrı 0600 dosyadadır. AutoSign'ın standart Certbot hesap/sertifika dosyaları özel izinlerle ACME ağacında tutulur; bu dosyalar ayrıca veritabanı kasasıyla şifrelenmez. Yedekleme ikisini de kapsar.

Denetim kaydında uygulamanın UPDATE/DELETE yetkisi yoktur. Hazır iş bulma SQL işlevleri sabit arama yolu kullanır ve yalnız opak iş/kiracı kimliklerini döndürür; iş verisi kiracı işlemi içinde okunur.

## Yapılandırma ve işler

Yapılandırma değişikliği yeni `configCode` üretir; agent durum yanıtında sürümü görüp yeniden `configure` ister. Veritabanına kaydetme, fiziksel sunucuda aynı anda uygulanma garantisi değildir. Grup profil ataması ve toplu değişiklikler tek veritabanı işlemindedir; çevrimdışı üyeler yeniden bağlanınca alır.

HTTP-01 ve harici yayım işleri kalıcı, süreli iş kirasıyla çalışır. Kesinti sonrası tekrar gönderim mümkündür; harici POST hedefi buna uygun olmalıdır. SSH kurulumunun durumu kaydedilir fakat yarıda kalan kurulum otomatik tekrarlanmaz.

## VPN oturumları ve muhasebe

Cihaz sayımı/oturum açma anahtar satır kilidi altında yapılır. Oturum sonlandırma ve politika değişimi kalıcı bekleyen bildirim oluşturur; agent kapanışı onaylayana kadar karar tekrar iletilir. Kota bildirimin sıklığına bağlıdır. Tekil kullanım rapor kimliği olmadığından kesintilerde tam bir kez muhasebe garanti edilmez.

Aynı kiracı grubunda uygun konum ve kapasiteye göre yönlendirme yapılır. Sunucu sırları ve sertifikaları ortak kimliğe dönüştürülmez. İlk adrese erişilemediğinde grup adres yayımı kullanılabilir. [Protokol](protokol-uyumlulugu.md), [grup profili](grup-sunucu-profili.md), [harici yayım](harici-adres-yayimi.md).
