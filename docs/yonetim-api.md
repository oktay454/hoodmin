# Hoodmin JSON yönetim API'si

Bu, Hoodmin'in bağımsız API'sidir. Kapalı kaynak Console'un özel API sözleşmesinin kopyası değildir. VpnHood sunucularının kullandığı `/api/agent/` ayrı ve VpnHood uyumludur.

## Kimlik ve yetki

Yönetici, **Panel kullanıcıları → Botlar ve API anahtarları** bölümünde bir bot oluşturur. Rol ve 1–365 günlük süre açıkça seçilir; anahtar yalnız oluşturulunca gösterilir. Veritabanında SHA-256 özeti tutulur. İptal veya süre sonu sonraki istekte geçerlidir. Botun parolayla tarayıcı oturumu açması mümkün değildir.

İstek başlığı `Authorization: Bearer <bot-anahtarı>`, JSON gövdelerde `Content-Type: application/json`. Anahtarı URL'ye koymayın. API kökü `/api/manage`.

| Rol | Yetki |
|---|---|
| owner / Sahip | İnsan üye; yönetici yetkileri ve sahip ataması. Son etkin sahip korunur. Bot olamaz. |
| admin / Yönetici | Proje, ekip, botlar ve tüm yönetim işlemleri; sahip rolünü değiştiremez. |
| operator / Operatör | Sunucu, grup, sertifika, kurulum, profiller, erişim ve oturum yönetimi; ekip/bot yönetemez. |
| token_writer / Erişim anahtarı yazarı | Erişim anahtarı/profili/kodu, anahtar indirme ve VPN oturumlarını kapatma; sunucu ayarını değiştiremez. |
| viewer / Salt okunur | Liste, durum ve korunmuş gelişmiş ayarlar; erişim sırrını indiremez, değişiklik yapamaz. |

Mevcut hesapların rolü kendiliğinden yükseltilmez. İlk `owner` hesabı güvenilir işletmen tarafından yerel `create-account --role owner` komutuyla oluşturulabilir. Botun rolünü değiştirmek için yeni bot oluşturup eskisini iptal edin.

Çerezle giriş yapan tarayıcı hâlâ CSRF ve Origin denetiminden geçer. API yalnız doğrulanmış bot Bearer kimliği kabul eder; yanlış Bearer varsa tarayıcı çerezine geri düşmez. Bot yanıtları önbelleğe alınmaz; her istek kiracı politikası ve güncel rolle denetlenir.

## Okuma

`GET /servers`, `/tokens`, `/sessions`, `/farms`, `/profiles`, `/server-profiles`, `/users`, `/bots` listeleri `{items, page, hasMore}` döndürür. Sayfa sıfırdan başlar; `?p=1` sonraki 100 kaydı ister. Kullanıcı ve bot listesi yönetici ister. Sunucu şifreli yapılandırması, token secret, oturum anahtarı ve bot özeti liste yanıtlarına girmez.

- `GET /servers/{id}/configuration`: `base_code`, düzenlenebilir `settings`, uygulama `status`.
- `GET /servers/{id}/credential`: sunucu bağlantı kimliği yenilemesi için güncel `confirmation`; operatör yetkisi gerekir.
- `GET /tokens/{id}/key`: `{value: "vh://..."}`; anahtar yazma yetkisi gerekir.
- `GET /tokens/{id}/download`: metin dosyası; anahtar yazma yetkisi gerekir.
- `GET /install-jobs/{id}`: `status`, çevrilmiş `label`, sırları gizlenmiş `log`.

## Yazma

POST işlemleri paneldeki aynı işlevleri, kilitleri, denetim kaydını ve doğrulamayı kullanır. Tarayıcıya özgü `csrf` alanı bot gövdesinde gerekmez. UUID'ler yalnız botun kiracısında çözülür; başka kiracının kaydı 404 olur. Genel başarısızlıklar JSON `error` ve uygun HTTP durumudur. Gövde tür/boyut hataları Axum'un 400/413/422 durumlarını kullanır.

| POST yolu | Gerekli veya anlamlı alanlar |
|---|---|
| `/servers` | `name`, `host`, `port`, `farm` (boş metin veya kendi grup UUID'si); yanıt `id`, bağlantı `settings` |
| `/servers/{id}` | `name`, `farm`, `paused`; isteğe bağlı `host`, `port` |
| `/servers/{id}/configuration` | `base_code`, `settings`, `confirmed:true` |
| `/servers/{id}/routing` | `base_code`, `policy`, `advertisement`, `confirmed:true` |
| `/servers/{id}/certificates` | `base_code`, `pfx` Base64, `password`, `tls_name`, `make_default`, `confirmed:true` |
| `/servers/{id}/challenge` | `token`, `key_authorization`, `confirmed:true` |
| `/servers/{id}/publish` | boş nesne |
| `/servers/{id}/credential` | `confirmation`, `confirmed:true`; kimlik yenilenince hedef ayarı da güncellenmeli |
| `/servers/{id}/installation` | `host`, `port`, `username`, `fingerprint`, `password` **veya** `private_key`; gerekirse `sudo_password`, eski kayıtta `credential`; `confirmed:true` |
| `/farms` | `name` |
| `/farms/{id}/publication` | `version`, `confirmed:true`; grup anahtarları için adres yenilemeyi açar |
| `/farms/{id}/certificates` | `base_code` = grup `version`, `pfx` Base64, `password`, `tls_name`, `make_default`, `confirmed:true`; mevcut/yeni üyelere ortak PFX dağıtır |
| `/tokens` | `server`, `name`, `count` 1–100, `max_clients` 0–10000, `max_gib`, isteğe bağlı `profile`, RFC3339 `expires`; yanıt `keys:[{id,name,key}]` |
| `/tokens/bulk` | boşlukla ayrılmış `ids`, `action` revoke/enable/quota/expiry; işlemine göre `max_gib`, `max_clients`, `expires` |
| `/tokens/{id}/policy` | `policy`, `confirmed:true`; etkin oturumları kapatır |
| `/tokens/{id}/details` | `name`, `group_name`, `description`, `customer_id`, `order_id`, `support_id` |
| `/tokens/{id}/access-code` | `confirmed:true`; yanıt `value` |
| `/sessions/{id}/close` | boş nesne |
| `/profiles` | `name`, `policy` |
| `/profiles/{id}` | `name`, `policy`, `apply_existing`; Public/Private türü sonradan değişmez |
| `/server-profiles` | `name`, `settings`, `management` |
| `/server-profiles/{id}` | `name`, `settings`, `management`, `version` |
| `/server-profiles/{id}/apply` | `version`, boşlukla ayrılmış `servers`, `confirmed:true` |
| `/users` | `email`, 16–128 bayt `password`, `role`; yönetici gerekir |
| `/users/{id}` | `role`, `active`; sahip koruması uygulanır |
| `/tenant` | `name`; yönetici gerekir |
| `/integrations` | `confirmed:true`; ödül doğrulama sırrını oluşturur/yeniler; yönetici gerekir |
| `/bots` | `name`, `role`, `lifetime_days`, `confirmed:true`; yönetici gerekir |
| `/bots/{id}/revoke` | `confirmed:true`; yönetici gerekir |

`settings`, `policy`, `management`, `advertisement` doğrudan JSON nesnesi veya JSON içeren metin olabilir. Alan adları kaynak sözleşmesiyle aynı olduğundan gelişmiş ayarlarda `Session`, `Tracking`, `NetFilter` büyük harfle yazılır. Miktarlar erişim politikasında bayt/Mbps/saniye; basit anahtar formunun `max_gib` değeri GiB'dir.

Kaynak değiştiren işlemler 202 `{status:"accepted", location:...}` döndürebilir. Bu, veritabanına kayıt onayıdır; VPN sunucusunda uygulanmış olduğunun kanıtı değildir. Yapılandırma `status.configCode` ve dinleyici sonuçlarıyla, SSH işi kendi durum yoluyla takip edilir. Anahtar/kimlik üreten işlemler 200 ile tek seferlik sırları döndürür. Eski yapılandırma veya sunucu profil sürümü 409 olur.

Sunucu yapılandırması, sertifika, kurulum, bot yetkisi ve kimlik yenileme işlemlerinde açık `confirmed:true` veya güncel kimlik yenileme kanıtı gerekir. Rol değişikliği, kurulum ve bağlantı kimliği yenileme otomasyonunda bu sonuçları işletmen değerlendirmelidir; sunucu kaydı silme veya genel kabuk komutu API'si yoktur.

## Harici adres yayımı

Grup yayım hedeflerinin listeleme/oluşturma, tekli/toplu çalıştırma, sürümlü düzenleme ve kaldırma uçları eklendi. Liste sırları içermez; hedef ayarı yönetici, yayımlama operatör yetkisi ister. [Yollar ve istek alanları](harici-adres-yayimi.md#api).
