# Grup sunucu profili

Sunucular → grup adı → **Grup sunucu profili** yolunda kullanılacak profili seçip ilgili onay kutusuyla **Ata ve uygula** düğmesine basın. Profiller **Sunucu profilleri** ekranından oluşturulur. Her gruba bir profil atanır; aynı profil birden çok gruba atanabilir. Atama ve profil sürümü birlikte denetlenir; eski ekran 409 ile reddedilir.

## Uygulama davranışı

- Atama mevcut tüm üyelerin yapılandırmasını ve yönetim kapasite/konum politikasını tek PostgreSQL işleminde günceller.
- Yeni kayıt veya başka gruptan taşıma, hedef grubun güncel profilini aynı kayıt işlemi içinde uygular. Profil kaydı ve üyelik değişimi kiracı düzeyinde kilitlenir; eşzamanlı yeni üyeler eski profilde kalmaz.
- Bağlı profil düzenlendiğinde ayrıca toplu etki onayı gerekir. Bağlı bütün gruplar ve üyeleri tek işlemde güncellenir. Bir kaydın şifreli ayarı okunamazsa işlem bütünüyle geri alınır.
- Sunucu kimliği, sırrı, sertifikaları ve istemciye duyurulan adres korunur. Dinleyici/ağ/kapasite ayarları değişebilir. Dinleyici portunu ortak profil belirliyorsa sunucunun duyurulan portu ve güvenlik duvarıyla uyumunu ayrıca sağlayın.
- Çevrimdışı sunucunun yeni yapılandırması veritabanında kalır; sunucu yeniden bağlandığında normal agent yapılandırma/sürüm onayı yolundan alır. Veritabanı işlemi atomiktir; bütün fiziksel sunucuların aynı anda yeniden yapılandırılacağı garantisi yoktur.
- Tekil gelişmiş ayar veya tek seferlik toplu uygulama hâlâ kullanılabilir; sonraki bağlı profil kaydı bu ayarları değiştirir. Bu sürümde alan bazında kalıtım/override katmanı yoktur.
- **Atamayı kaldır** otomatik uygulamayı keser; son ayarları geri almaz. Profilsiz başka gruba veya grup dışına taşınma da son ayarları korur. Eski ayarlar gerekiyorsa ayrıca açıkça uygulanır.
- Sertifika deposu ve harici adres yayımı ayrı denetimlerdir. Profil kaydı harici yayıncıyı hemen tetiklemez; onun zamanlayıcısı/elle yayımı kullanılır.

## API ve yetki

Okuma için viewer; atama/değişiklik için operator/admin/owner gerekir. Botlar aynı kiracı, sürüm ve onay denetimlerini kullanır.

- GET /api/manage/farms/{id}/server-profile: grup sürümü, atanmış profil, üye sayısı ve seçilebilir profillerin kimlik/sürümleri.
- POST /api/manage/farms/{id}/server-profile: version, profile, profile_version, confirmed:true. Kaldırma için profile boş dize; profil sürümü kaldırmada gerekmez.
- POST /api/manage/server-profiles/{id}: mevcut alanlara ek olarak bağlı gruplar varsa confirmed:true.

Şema geçişi 0010_farm_server_profiles.sql, paket sürümü 0.1.0-5~deb13u1. Eski QA üzerinde sınanır; yeni üretim bu çalışma sırasında yükseltilmez.
