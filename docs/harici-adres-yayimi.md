# Harici adres yayımı

Sunucular → sunucu grubunda **Grup adresi yayımı** → **Harici adres yayımı**. Kaydetme/düzenleme ve kaldırma yönetici/sahip rolü gerektirir. Operatör etkin hedefleri tek tek veya topluca sıraya alabilir; gözlemci ve erişim anahtarı yazarı yalnız durum listesini okur. Bot API aynı doğrulayıcıları kullanır.

## Hedefi oluşturma

1. Başlangıç sunucusunu seçin. Eski erişim anahtarının sırrı bu sunucuya aittir; dosya o sırla şifrelenir. İçerik uygun canlı grup üyesinin kendi adresi, sertifika parmak izi ve sırrıdır. Her farklı başlangıç sunucusu için ayrı dosya/yayım hedefi kullanın. Aynı kiracıda aynı dosya adresinin farklı başlangıç sunucularına atanması reddedilir.
2. **Yayım adresi**, istemcilerin dosyayı okuyacağı genel HTTPS adresidir. **Yükleme adresi**, dosya/CDN API'sinin PUT/POST hedefidir. Bunlar farklı olabilir. Yükleme kimliği dosyayı okuyan GET isteğine gönderilmez.
3. PUT, POST veya **PUT → POST** seçin. Son kipte PUT başarısızsa aynı gövdeyle POST denenir; başarılı PUT'tan sonra POST gönderilmez. HTTP yönlendirmeleri izlenmez.
4. Yayıncı kimliğini girin. Başlık örneği: `Authorization: Bearer {access_token}`. Alanlar `Ad: Değer` satırlarıdır; Host, Content-Length ve hop-by-hop/vekil başlıkları kullanıcı tarafından yönetilemez. İçerik türünü ayrı alandan seçin.
5. Ham dosya yüklemesinde Body=`{farm_token}`, içerik türü `text/plain; charset=utf-8` ve form verisi boş olmalıdır. Otomatik JSON sarmalama yapılmaz. Bir JSON API'si gerekiyorsa kendi API sözleşmesine uygun gövde şablonunu ve içerik türünü açıkça girin.
6. Form yüklemesinde Body'yi boş bırakıp örneğin `payload: {farm_token}` ve `key: {access_token}` satırlarını kullanın. İstek multipart/form-data olarak oluşturulur; içerik türü ve sınır ayracı otomatik belirlenir. Hedef API'nin payload alanını dosya içeriği olarak yayımlaması gerekir.
7. Otomatik aralık 0 ise yalnız kaydetme/elle başlatma iş oluşturur. 5–1440 dakika aralığında düzenli yayım yapılır. Etkin bir hedefi kaydetmek ilk işi hemen sıraya alır. Gönderimi onaylayıp kaydedin.

`{farm_token}` şifreli adresle, `{access_token}` yayıncı kimliğiyle değiştirilir. URL'deki kimlik URL kodlamasıyla eklenir. Console'da gözlenen URL yer tutucusu hatası ve ham metne JSON içerik türü verme davranışı Hoodmin'e kopyalanmadı. Gövde ve hedef API sözleşmesi korunur.

## Durum ve erişim anahtarları

İş, yükleme başarısından sonra yayım adresini GET ile okur ve dosyanın şifreli metinle bayt düzeyinde aynı olduğunu doğrular. Ancak bundan sonra **Başarılı** olur ve adres, seçilen başlangıç sunucusundan üretilen yeni **grup erişimli** anahtarlara eklenir. Başka başlangıç sunucularının sırları için aynı dosya kullanılmaz. Eski anahtarlar yeniden paylaşılmalıdır. İlk başarılı dosya da gerektiğinde aynı harici yenileme adresini korur.

Hedef çalışırken düzenleme/kaldırma reddedilir; eski sürümlü formlar 409 verir. Etkinliği kaldırmak sonraki zamanlamayı durdurur ve yeni anahtarlara adres eklenmesini keser. Kaldırma, şifreli yayıncı kimliğini veritabanından siler. Önceden paylaşılmış anahtarlardaki URL ve harici dosya otomatik silinmez. Bunlar hedef hizmette ayrıca yönetilir. Sıradan bir başarısız yenileme son başarılı adresi yeni anahtarlardan kendiliğinden kaldırmaz; son çalışan dosya korunur.

CDN önbelleği nedeniyle dosya henüz yenilenmediyse iş başarısız görünür. Önbellek süresi veya hedef API'yi kontrol edip yeniden yayımlayın. Hata yanıtının gövdesi ve kimlik başlıkları kullanıcıya/günlüğe taşınmaz; HTTP durum kodu veya sınırlandırılmış hata sebebi gösterilir. Son deneme ve son başarı zamanları UTC'dir.

## Görevlerin işletimi ve sınırlar

- Ayarlar, kiracı/hedef kimliğine bağlı AES-GCM bağlamıyla şifrelenir; yükleme URL'si, kimlik ve şablonlar kayıtlı hâliyle arayüze geri verilmez. Değiştirmek için tüm yükleme alanlarını yeniden girin; eski kimlik başka bir yükleme adresine kendiliğinden taşınmaz.
- Kalıcı sıranın iş kirası 120 saniye, iş zaman aşımı 90 saniyedir. Servis yeniden başladıktan sonra süresi dolmuş işler tekrar alınır. Bu **en az bir kez** (at-least-once) davranışıdır; sunucu isteği kabul ettikten hemen sonra süreç çökerse istek tekrarlanabilir. POST hedefi tekrarlanan isteklere uygun tasarlanmalıdır.
- İşçi iki saniyede bir hazır kimlikleri bulur; hazır bir işi alma atomiktir. Otomatik işler ayarlanan aralıkta yeniden çalışır; 0 aralıklı başarısız işler elle tekrar başlatılır. Yapılandırma değişikliği ayrıca otomatik olay tetiklemez; aralık veya elle yayım kullanılır.
- En fazla 16 hedef/grup; en fazla 32 başlık/form alanı; gövde ve yanıt 256 KiB; bağlantı 5, tek HTTPS isteği 20 saniye. Sıra seri işlenir; sıra bekleme süresi işin 90 saniyelik yürütme süresine dahil değildir.
- Yalnız genel HTTPS hedefleri kabul edilir. Her bağlantıda DNS sonuçlarının tümü denetlenir; özel, yerel, metadata, geçiş ve ayrılmış adresler reddedilir. Doğrulanan bir adres curl'e sabitlenir; DNS tekrar bağlama önlenir. Ortam vekilleri, kullanıcı curlrc dosyası ve yönlendirmeler kullanılmaz. TLS sertifikası doğrulanır. Bu sürüm ilk doğrulanan genel DNS adresini kullanır; diğer DNS kayıtları arasında otomatik bağlantı geçişi yapılmaz.
- Geçici gövde/yanıt dosyaları 0700 iş dizinindedir; işlem sonunda kaldırılır. Kimlikler ve URL'ler süreç argümanlarına yazılmaz, curl'e standart girdiden geçirilir.
- Zamanlayıcının güvenlik tanımlayıcılı SQL işlevi yalnız hazır işlerin opak kiracı/hedef kimliklerini döndürür; şifreli ayarları döndürmez. İşin tüm veri erişimi kiracı RLS işlemi içinde yapılır. Veritabanı uygulama rolü süper kullanıcı veya BYPASSRLS değildir.

curl davranışı için birincil başvuru: [resmî curl belgesi](https://curl.se/docs/manpage.html) (`--resolve`, `--config`, `--proto`, `--max-time`, `--max-filesize`). Paket en az curl 8.4 gerektirir; Debian 13 hedeflenir.

## API

| İşlem | Yol |
|---|---|
| Liste | `GET /api/manage/farms/{farm}/destinations` |
| Oluştur | `POST /api/manage/farms/{farm}/destinations` |
| Düzenle | `POST /api/manage/publications/{id}` |
| Tekli yayımla | `POST /api/manage/publications/{id}/push` |
| Toplu yayımla | `POST /api/manage/farms/{farm}/push` |
| Kaldır | `POST /api/manage/publications/{id}/remove` |

Tüm API yolları bot Bearer kimliği gerektirir. Liste yalnız güvenli metaveriyi döndürür. Tekli yayım/kaldırma `version` ve `confirmed:true`, toplu yayım `confirmed:true` ister. Oluşturmada `name`, `seed`, `publish_url`, `upload_url`, `method` (`PUT`, `POST`, `PUT_POST`), `secret`, `headers`, `form_data`, `body`, `content_type`, `enabled`, `auto_minutes`, `confirmed` kullanılır. Düzenlemede ayrıca `version` gerekir. `replace_settings:false` kayıtlı yükleme ayarlarını korur; `true` tüm yükleme alanlarını değiştirmek içindir. Başarılı yazma yanıtı 202 ve izlenecek konumdur; liste 200 döner.

## Doğrulama ve dağıtım

`tests/test_external_publications.py`: gerçek genel HTTPS ham ve multipart yükleme, PUT geri dönüşü, başarılı PUT sonrası POST'un atlanması, URL/başlık/form şablonları, byte denetimi, bot API/roller, RLS, şifreli kimlik, kapatma/kaldırma, sürüm çakışması, süreli iş kirası ve servis yeniden başlatması. Harici dosya gerçek .NET `ServerToken.Decrypt` ile çözüldü ve hazır VPN sunucusuna gerçek istemci bağlantısı geçti.

Bu özellik eski QA ortamında uygulanır. Yeni üretim makineleri bu kilometre taşında değiştirilmez. Debian `0.1.0-4~deb13u1` paketi `0009_external_publications.sql` geçişini içerir. Paket yükseltmesinde bakım/yedekten sonra hizmeti durdurun, paketi kurun, root ile `hoodmin-db-setup` çalıştırın ve hizmeti başlatın; ana anahtar ve mevcut ortam dosyasını koruyun. Önceki üretim paketi ve ortam bilgisi [üretim ortamı belgesindedir](uretim-ortami.md).
