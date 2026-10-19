# Console harici adres yayımı sınaması — 19 Ekim 2026

Kullanıcı geçici QA alıcısı, Console deneme çiftliğinin şifreli adresinin bu alıcıya gönderilmesi ve yalnız bu işin deneme kaydının kaldırılmasına açık onay verdi. Eski `yapayzeka` ortamı ve `vhm-test.oktayaktogan.com.tr/__hoodmin_publish_qa/…` kullanıldı. Yeni üretim makinelerinde, güvenlik duvarında ve UID/GID eşlemesinde değişiklik yapılmadı. Yeni paket kurulmadı.

## Görünen alanlar

Server Farms → Server Farm 1 → Farm Token Urls: Publish Url, Name, Auto Upload; yöntemler Off / POST / PUT / PUT & POST. Yöntem seçilince ayrıca Access Token, File Url, Headers, Form-Data ve Body açılır. Liste Status, Auto Publish, Last Pushed ve Error gösterir. PUSH ALL açık uyarı/onay adımı içerir.

Formun yer tutucuları:

- File Url: `https://cdndomain.com/my-project/farm-token?api_key={access_token}`.
- Headers: `Authorization: Beraer {access_token}` ve `Key: Value` satırları; `Beraer` yazımı Console'un kendi örneğidir.
- Form-Data: `Key: Value` satırları.
- Body: `{farm_token}`.

Publish Url okuma adresi, File Url yükleme adresidir. Sınamada gerçek bir yayıncı API anahtarı yerine yalnız zararsız bir işaret değeri kullanıldı. Gerçek Console kimlikleri veya yönetim sırları alıcı başlıklarına konmadı.

## İlk aşama: ModSecurity etkinken gözlenen istekler

| Durum | Gözlem | Sonuç |
|---|---|---|
| Body=`{farm_token}` | VPS ModSecurity `200002`, JSON çözümleme hatası | 400'ün kaynağı kullanıcı tarafından sağlanan hata günlüğüyle belirlendi. |
| Headers içine `Content-Type: text/plain` ekleme | Console: `Content-Type does not support multiple values` | Bu Console akışında içerik türü bu alanla değiştirilemedi. |
| Body=`"{farm_token}"`; PUT & POST; alıcı PUT=405, POST=201 | PUT sonra POST; iki gövdenin SHA256 özeti aynı; `application/json; charset=utf-8` | Başarısız PUT'tan sonra POST'a geçiş doğrulandı. |
| Aynı kip, alıcı PUT=201 | PUT ardından GET; POST gelmedi | İki yöntem koşulsuz birlikte gönderilmiyor. Başarılı PUT sonrası POST atlanıyor. |
| JSON yükleme → alıcıda dizeyi açıp düz metin GET sunma | Console listesinde **OK** | Yükleme ve yayımlanmış içeriğin denetimi başarıyla tamamlandı. |
| `X-Hoodmin-QA: {access_token}` | Alıcıda gerçek değer yazdırılmadan işaret eşitliği `true` | Başlık yer tutucusu değiştiriliyor. |
| Body=`"{farm_token}"` | Geçerli JSON dizesi; açılan metin `Base64(IV).Base64(ciphertext)`; IV 16 bayt, şifreli bölüm 448 bayt | Gövde yer tutucusu şifreli çiftlik adresiyle değiştiriliyor. İçerik bu sınamada ayrıca çözülmedi. |
| File Url sorgusunda `qa={access_token}` | Alıcıda sorgu değeri hâlâ tam `{access_token}` olarak sınıflandırıldı | Bu sürümde URL yer tutucusu için formdaki örnekle gerçek davranış uyuşmadı. |
| Form-Data=`payload: {farm_token}` ve `key: {access_token}`, Body boş, POST | 400; alıcıya form gövdesi ulaşmadı | Form kodlaması/yer tutucuları teyit edilmedi; neden bu denemenin kaydıyla kesinleşmedi. |

Birleşik kipte 405→POST ve 201→dur davranışı doğrulandı; zaman aşımı, yönlendirme, tüm hata kodları ve tekrar deneme takvimi hakkında sözleşme iddia edilmez. Console'da ekleme/güncelleme sonrası otomatik deneme ve PUSH ALL ile elle tetikleme görüldü; uzun süreli otomatik yayım sıklığı ayrıca ölçülmedi.

Başlangıçtaki ModSecurity hatası, şifreli düz metnin JSON içerik türüyle gönderilmesinden kaynaklandı. Güvenlik duvarı kapatılmadı veya kural istisnası eklenmedi. Geçerli JSON dizesi gövdesiyle bu sınama engeli giderildi. Yükleme API'sinin JSON dizesini dosya içeriğine dönüştürmesi alıcının sınama davranışıdır; her CDN'nin aynı API'yi sunduğu anlamına gelmez.

## Alıcı ve temizlik

`tests/publication_sink.py` Python standart kitaplığıyla 127.0.0.1:18081 üzerinde çalıştı. Rastgele yol ve 64 KiB üst sınırı kullanıldı. Bir saat ömürlü geçici systemd hizmeti `nobody`, NoNewPrivileges ve salt okunur sistem korumasıyla başlatıldı; yalnız geçici çalışma dizini yazılabildi. Nginx yolu için önce dosyanın birebir kopyası alındı. Normal hizmetler bu alıcıya yönlendirilmedi.

İsteklerde yöntem, durum, içerik türü, uzunluk, özet ve başlık **adları** kaydedildi; başlık değerleri yazdırılmadı. Yer tutucular yalnız doğru/yanlış sınıflandırmasıyla kontrol edildi. Şifreli gövdeler sınama süresince özel geçici dizinde tutuldu ve sonunda kaldırıldı. Git dışında eski QA `work/console-publication-qa-20261019-success.json` dosyasında yalnız metaveri korundu.

Her iki geçici Console kaydı da kaldırıldı; liste yeniden boş doğrulandı. Nginx dosyası bayt düzeyinde önceki hâline getirildi; geçici hizmet, yol ve veri dosyaları kaldırıldı. Son denetimler: alıcı portu kapalı, geçici yol 404, QA giriş sayfası 200, Nginx ve panel hizmetleri etkin.

## Kullanıcının uyumluluk kararı

Kullanıcı ModSecurity'yi sınama FQDN'i için kapatabileceğini belirtti ve güvenlik duvarına uydurmak amacıyla standart veri biçiminin değiştirilmemesini istedi. Yukarıdaki JSON dizesi ve alıcıda JSON açma işlemi yalnız yöntem davranışını gözlemek için geçici teşhis düzeniydi; Hoodmin'e uygulanmış bir ürün davranışı veya varsayılan yayım biçimi değildir.

Ham şifreli metin, gövdeyle uyumlu içerik türü ve hedef API'nin açıkça gerektirdiği form/gövde şablonu esas alınacak. WAF engelini aşmak için otomatik JSON sarmalama/açma eklenmeyecek. Kullanıcı ardından bu FQDN için ModSecurity'yi kapattığını bildirdi; aşağıdaki tekrar sınaması tamamlandı.

## İkinci aşama: ham gövde ve Form-Data doğrulandı

Kullanıcı ModSecurity'yi `vhm-test.oktayaktogan.com.tr` için kapattığını bildirdikten sonra aynı açık onay kapsamında tek geçici Console kaydı ve eski QA alıcısı yeniden açıldı. Güvenlik duvarına asistan tarafından müdahale edilmedi.

- **Ham gövde:** Body=`{farm_token}`, Form-Data boş. PUT=405 → POST=201 → GET=200; PUT/POST gövdeleri aynı 625 baytlık şifreli metindi. Tırnak, JSON sarmalaması veya alıcıda JSON açma yoktu. IV 16 bayt, şifreli bölüm 448 bayt; Console durumu **OK** oldu. Console, gövde JSON olmadığı hâlde `application/json; charset=utf-8` başlığı gönderdi; veri biçimi ile içerik türü arasındaki uyumsuzluk doğrudan görüldü.
- **Form-Data:** Body boş; `payload: {farm_token}` ve `key: {access_token}` satırları; yöntem POST. Alıcıya **multipart/form-data** ve iki alan ulaştı. Gövde 959 bayttı; `payload` içindeki şifreli metin 625 bayt, `key` sahte işaret değeriyle eşleşti. Böylece her iki yer tutucu da form alanlarında doğrulandı.
- **Form yüklemesinin okuma denetimi:** Normal bir dosya yükleme API'si gibi, alıcı multipart `payload` alanını dosya içeriği olarak sakladı; gelen multipart gövde değiştirilmeden ayrıca gözlendi. Yayımlanan dosyanın `payload` ile bayt düzeyinde aynı olduğu kontrol edildi. GET=200 ve Console **OK** görüldü. Bu, JSON sarmalama çözümü değildir; form alanının dosya olarak yayımlanmasıdır.

Form yükleme akışında yükleme adresine özel işaret başlıklı ek bir GET de görüldü. Bunun tüm amaçları veya her depolama sağlayıcısı için zorunluluğu henüz genellenmez. İlk aşamadaki Form-Data belirsizliği bu tekrar sınamasıyla giderildi; URL sorgusu yer tutucusu ve uzun süreli otomatik yayım sınırları ilk aşamadaki gibi kalır.

Sınama sonunda tek geçici kayıt silindi ve liste boş doğrulandı. Nginx dosyası tekrar aynen geri yüklendi; alıcı, yol ve yakalanan gövdeler kaldırıldı. Gizli içermeyen yöntem/durum/karşılaştırma kanıtı Git dışı `work/console-publication-qa-20261019-raw-form.json` dosyasında tutulur. Üretim makineleri değiştirilmedi; yeni paket kurulmadı.

## Hoodmin'e aktarım durumu

Bu çalışma gözlem ve sınama aracıdır; Hoodmin'e harici POST/PUT yayıncısı henüz eklenmedi. Çalışan mevcut `/f/…` ve `/s/…` şifreli adres servisleri korunur. Aktarımda ayrı okuma/yükleme adresleri, şifreli yayıncı kimliği, başlık/gövde şablonları, yöntem geri dönüşü, elle/otomatik görevler ve hata/durum takibi gereklidir. Kiracı yalıtımı, yetki denetimi, sırların günlüklere sızmaması ve iç ağa istek atma (SSRF) koruması birlikte sınanmalıdır. Console'un gözlenen içerik türü/URL sorunlarını Hoodmin'de aynen tekrarlamak gerekmez; uyumlu sonuç üreten davranış ve farkları açıkça belgelenmelidir.
