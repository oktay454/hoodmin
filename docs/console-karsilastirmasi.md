# Hoodmin — console.vpnhood.com karşılaştırması

Değerlendirme tarihi: 16 Ekim 2026. Bu belge geliştirme onayı veya yeni özellik uygulaması değildir.

## Sınıfların anlamı ve kanıt sınırı

- **Doğrudan:** Resmî VpnHood Manager açıklamasında kullanıcıya sunulan bir işlevin karşılığı var. İç uygulama yönteminin aynı olduğu anlamına gelmez.
- **Mantıksal:** Hoodmin'in kendi sunucusunda güvenilir çalışması için gereken mühendislik/işletim işi. Kapalı kaynak Manager'ın iç mimarisi hakkında iddia değildir.
- **Eklense güzel olur:** Mevcut BYOS hedefini tamamlamak için zorunlu olmayan kullanım kolaylığı veya kapsam genişletmesi. Console'da doğrudan karşılığı olan bir özellik de bizim kapsamımızda isteğe bağlı olabilir.

Bu turda açık bir console oturumu bulunmadığından güncel menüler yeniden denetlenmedi. Karşılaştırma resmî [Manager ürün açıklaması](https://www.vpnhood.com/vpnhood-manager/) ve [Manager wiki](https://github.com/vpnhood/VpnHood/wiki/VpnHood-MANAGER) üzerinden yapılmıştır. Ekran konumu ve iç mimari hakkında doğrulanmamış ayrıntı verilmez. Ürünün kaynak kodu incelenebilir olmadığından iş kuyruğu, yedekleme ve muhasebe garantileri hakkında eşdeğerlik iddiası yoktur.

## Önceki kalan işler listesinin karşılığı

| İş | Sınıf | Console ile bağlantısı | Hoodmin'de durum / gerekçe |
|---|---|---|---|
| Debian paketi (.deb) | Mantıksal | Console bir hizmettir; kullanıcısına Hoodmin kurulum paketi sunmasıyla birebir karşılık beklenmez. | Kendi altyapımıza dağıtım, bağımlılık/servis yönetimi, veri koruyan yükseltme ve kaldırma için gerekli. Henüz paket yok. |
| SSH veya tek komutla sunucu kurma | Doğrudan | Resmî açıklama tek komutla kurulum ve SSH bilgileriyle kurulum sunuyor. | Panel şu anda bağlantı ayarını üretir; kullanıcı elle uygular. Resmî hazır VpnHood kurulumunu kullanarak otomasyon eklenebilir. |
| SSH ile bakım/güncelleme | Mantıksal; isteğe bağlı otomasyon | Otomatik kurulum doğrudan doğrulanıyor; kapsamlı bir SSH bakım/güncelleme ekranı bu kaynaklarda ayrıca doğrulanmıyor. | Güvenli güncelleme prosedürü gerekli; bütün bakımın panelden yapılması zorunlu değil. VpnHood'un resmî güncelleyicisi ile çakışmamalı. |
| Kalıcı iş kuyruğu (job queue) | Mantıksal | Kullanıcıya sunulan kurulum gibi uzun işler vardır; hangi kuyruk teknolojisinin kullanıldığı bilinmiyor. | SSH/ACME gibi uzun işler eklendiğinde kesinti sonrası sürdürme, durum takibi, yeniden deneme ve eşzamanlılık kontrolü için. Basit mevcut form işlemlerinin hepsi kuyruk gerektirmez. |
| Sunucu çiftliği ve yük dağıtımı (server farm / load balancing) | Doğrudan | Resmî açıklama tek anahtarla çiftlikteki sunuculara erişimi ve ağırlıklı dağıtımı anlatıyor. | Gruplar yalnız düzenleme amaçlı; anahtar tek sunucuya bağlı. Yalnız aynı kiracının kendi sunucuları arasında uygulanmalı. |
| Anahtar bağlantısı (token URL) ve adres yenileme | Doğrudan | Resmî açıklama istemcilerin bağlantı üzerinden yeni adresi almasını anlatıyor. | IP/FQDN düzenlenebiliyor, fakat eski statik anahtarlar güncellenmiyor. Bağlantıdan yenileme henüz yok. |
| Otomatik sertifika yönetimi (ACME) | Doğrudan | Resmî açıklama çiftlikler için TLS sertifikası, Let's Encrypt ile alma/yenileme veya mevcut sertifikayı kullanma sunuyor. | VpnHood sunucusu/çiftliği sertifikasının panelden yönetimi yok. Mevcut Hoodmin web sitesinin VPS üzerindeki HTTPS sertifikası ayrı bir konu. |
| Yedekleme, geri yükleme ve felaket kurtarma | Mantıksal | Console'un işletim ayrıntıları bilinmiyor; görünür özellik olarak varsayılmıyor. | PostgreSQL, şifreleme anahtarı ve gerekli yapılandırmaların birlikte kurtarılması gerekir. Prosedür ve geri yükleme sınaması eksik. |
| Şifreleme anahtarı döndürme (key rotation) | Mantıksal | Kapalı kaynak ürünün anahtar yaşam döngüsü bilinmiyor. | Hoodmin'in verileri yeniden şifreleyerek ana anahtarını güvenli değiştirebilmesi gerekir. Mevcut sunucu bağlantı kimliği yenileme özelliği bunun yerine geçmez. |
| Gerçek VPN trafiği, uzun süreli çalışma ve yük sınaması | Mantıksal | Bir ekran özelliği değil, doğrulama işidir. | Protokol ve uzak bağlantı sınandı; NullVpnAdapter gerçek kullanıcı trafiğini taşımaz. Yönlendirme ve yük altında doğruluk ayrıca sınanmalı. |
| Kiracı oluşturma / çalışma alanı geçişi | Karma | Kuruluş/proje ayrımı bizim kiracı modeliyle ilişkilidir; yönetilen SaaS hizmetinin platform yöneticisi ekranını görmüş sayılmayız. | Kiracı oluşturma CLI'da. Aynı kullanıcının birden çok çalışma alanına geçişi istenirse arayüz gerekir. Kayıt/davet veya platform paneli iş akışına göre kararlaştırılmalı. Herkese açık kayıt zorunlu değil. |
| Kota muhasebesinin kesintilere dayanıklılığı | Mantıksal | Trafik kotasının doğrudan karşılığı vardır; Console'un tam bir kez muhasebe garantisi bilinmiyor. | VpnHood kullanım sözleşmesinde tekil rapor kimliği yok; kesintilerde eksik sayım mümkün. Önce arıza senaryoları ölçülmeli; yalnız panel tarafında kuyruk eklemek bu protokol sınırlamasını çözmez. |
| Ürün adı ve üç dilde benioku | Eklense güzel olur; ayrıca kullanıcı tarafından istendi | VpnHood Manager özelliğinin kopyası değil, bağımsız ürün kimliği ve belgelendirme işi. | Ürün adı Hoodmin; Türkçe, Arapça ve İngilizce README dosyaları hazır. |

## Önceki listede yeterince açık olmayan doğrudan farklar

Resmî açıklama planlar ve hız sınırı, bot/API anahtarları ve ayrıntılı kullanım görünümünü de içeriyor. Bunlar ilk kalan işler listesinde açıkça ayrı yazılmamıştı. Mevcut panelin tüm özelliklerde eşdeğer olduğu söylenemez.

| Özellik | Sınıf | Hoodmin'de durum |
|---|---|---|
| Tekrar kullanılabilir plan/profil, aylık trafik ve hız sınırı | Doğrudan | Anahtar başına trafik/cihaz/son tarih var. Plan şablonu, dönemsel kota yenileme ve hız sınırı yönetimi henüz yok. |
| Botlara API anahtarı ve yönetim REST API'si | Doğrudan | VpnHood sunucusunun agent API'si var; bu, kullanıcı/bot için yönetim API'si değildir. Bot kimlikleri, kapsamlı API anahtarları ve belgelenmiş yönetim sözleşmesi yok. |
| Ayrıntılı trafik geçmişi, grafikler ve ülke kırılımı | Doğrudan | Canlı sayaçlar, CPU/bellek/oturum durumu ve CSV var. Resmî açıklamadaki ayrıntılı gözlem/raporlama düzeyine ulaşılmadı. |
| Bulut sağlayıcı üzerinden yeni genel IP alma/bırakma | Doğrudan karşılığı var; bizim için eklense güzel olur | Console bunu desteklenen sağlayıcılarla sunuyor. Herkesin kendi sunucusunu getirdiği Hoodmin'de bulut API bilgileri ve sağlayıcı entegrasyonu gerektirir; elle IP düzenlemeye ek, isteğe bağlı kapsamdır. |

## Pazarlama sayfasının tüm vaatleri — ayrıntılı inceleme

Kullanıcının isteğiyle [Manager pazarlama sayfası](https://www.vpnhood.com/vpnhood-manager/) bu kez tarayıcıda bütünüyle okunarak giriş metni ve sekiz özellik kartı kaynak koduyla karşılaştırıldı. Bu, oturum açılmış Console menülerinin yeni bir denetimi veya kapalı kaynak uygulamanın davranış sınaması değildir. Sayfadaki genel vaatlerin çalıştığına dair bağımsız bir garanti verilmez.

Aşağıdaki satırlar **doğrudan ürün vaatlerinin** karşılaştırmasıdır. Eksik olmak, uygulanmasının bu turda onaylandığı anlamına gelmez.

| No | Sayfadaki vaadin özeti | Hoodmin durumu | Eksik / sınır / kaynak dayanağı |
|---|---|---|---|
| 1 | Kendi sunucularını tek panelden yönetme | Mevcut | BYOS ve tek kiracıya ait sunucu modeli var. `migrations/0001_initial.sql`, `0002_panel.sql`, `store.rs`. Manager'ın hizmet bedeli hakkındaki pazarlama ifadesinden Hoodmin için fiyatlandırma çıkarmıyoruz. |
| 2 | Arayüzü kendi markasıyla sunma | Eksik | Hoodmin adının sabitlenmesi, yönetilebilir marka özelleştirmesi değildir. `views.rs` marka metinleri sabit; `update_tenant` yalnız çalışma alanının adını değiştirir. Logo/renk gibi ayarlar düşünülebilir; kiracı başına marka veya özel alan adı desteği sayfada ayrıntılandırılmıyor. |
| 3 | Linux/Windows'ta tek komut veya SSH ile kurulum | Eksik | Yalnız Debian sınama betikleri var; panelden sunucu kurulum akışı yok. Resmî VpnHood hazır kurulumu kullanılarak geliştirilmeli. Windows burada VPN sunucusunun kurulum hedefidir; Hoodmin yönetim servisinin Windows'a taşınması şartı değildir. |
| 4 | İstemcinin anahtarı yapıştırarak bağlanması | Temel karşılık mevcut | `management::create_tokens` uyumlu `vh://` üretir; gerçek .NET/uzak bağlantı doğrulaması kayıtlı. Ancak gerçek VPN trafiği henüz sınanmadı. Anahtarları yeniden dağıtmadan güncelleme ayrı 7. satırdır. |
| 5 | Plan başına cihaz, aylık trafik, hız ve süre sınırları | Kısmen | Anahtar başına cihaz, toplam trafik ve bitiş zamanı mevcut. Plan şablonu, hız sınırı ve aylık kota dönemi yok. Aylık kotanın yenilenme zamanı/kuralları pazarlama metninde açıklanmıyor; bunlar ayrıca tasarlanmalı. İşlem başına 100 anahtar sınırı, toplam anahtar sayısının 100 olduğu anlamına gelmez. |
| 6 | Çiftlikte tek anahtar, ağırlıklı dağıtım, yeni sunucunun katılması | Eksik | Gruplar etiketleme/düzenleme içindir. `access_tokens.server_id` tek sunucuya bağlı; çiftlik üyeliğine göre anahtar, ağırlık ve seçim/yönlendirme mantığı yok. Yeni sunucunun nasıl ve hangi sürede trafiğe alınacağı ayrıca sınanmalı. Kiracılar arası ortak havuz gerektirmez. |
| 7 | Desteklenen sağlayıcıda IP değiştirme ve anahtar bağlantısıyla adres yenileme | Eksik | IP/FQDN düzenleme var; sağlayıcıdan IP alma/bırakma ve token URL yok. Önceden paylaşılan statik anahtarlar güncellenmiyor. Bulut IP otomasyonu isteğe bağlı kapsam olabilir; token URL'nin doğrudan karşılığı ayrı tutulmalı. |
| 8 | Çiftlik TLS'i, otomatik sertifika veya mevcut sertifikayı kullanma | Eksik; temel TLS mevcut | `make_config` sunucu başına kendinden imzalı sertifika üretir. Çiftlik sertifikası, Let's Encrypt/ACME ve kullanıcı sertifikası içe aktarma arayüzü yok. Sertifika dosyasının teknik yapılandırmada bulunması, kullanıcıya sunulan içe aktarma özelliği değildir. HTTPS benzerliği VpnHood tünelinin özelliğidir; Hoodmin VPN motorunu yeniden yazmaz. |
| 9 | Durum, etkin kullanıcılar, anlık hız, ülke ve kullanım görünümü | Kısmen | Durum/CPU/bellek, oturumlar, toplam bayt ve CSV var. Anlık trafik hızı, ülke kırılımı ve zaman serisi/ayrıntılı kullanım raporu yok. SSE özetleri 15 saniyede yeniler; bu tek başına anlık hız ölçümü değildir. `views.rs`, `events`, `export_csv`. |
| 10 | Ekip daveti ve rol verme; bot anahtarıyla tüm yönetim işlemlerine API erişimi | Kısmen | Admin/operator/viewer ve doğrudan hesap oluşturma var. Davet/kabul akışı, bot API anahtarı ve belgelenmiş JSON yönetim API'si yok. Sayfa davetin e-posta veya bağlantı türünü belirtmiyor. HTML form uç noktaları ile agent API'si, bu vaadin karşılığı sayılmaz. `add_user`, `web::router`, `agent::router`. |

### Önceki değerlendirmeye eklenen veya netleştirilen farklar

- Yönetilebilir marka özelleştirmesi (branding) önceki tabloda açıkça listelenmemişti. Bağımsız Hoodmin adı seçimi ile ayrı bir ürün özelliğidir.
- Ekip hesabı açma ve rol yönetimi mevcut; davet/kabul akışı ayrıca eksik. Davetin teknik yöntemi kaynakta açıklanmadığından e-posta zorunluluğu varsayılmadı.
- Kurulum hedeflerinde Windows da var. Debian önceliğimiz korunurken VPN sunucusu kurulum hedefi olarak kapsamı ayrıca seçilmeli.
- Çiftlik vaadi ağırlıkları ve yeni üyeyi kapsıyor; yalnız grup kaydı yeterli değildir.
- Sertifika tarafında otomasyona ek olarak mevcut kullanıcı sertifikasının içe aktarılması da eksik.
- Canlı toplam sayaç, anlık hız ve geçmiş raporu aynı şey değildir. Ülke kırılımı için gereken konum verisi kaynağı ve gizlilik tercihleri ayrıca tasarlanmalı.

### Pazarlama metninden çıkarmadığımız gereksinimler

Bu sayfa ödeme alma, fatura/abonelik tahsilatı, mağaza veya müşteri ödeme portalı vaat etmiyor. Plan tanımlama ifadesini otomatik ödeme sistemi zorunluluğu olarak yorumlamıyoruz. Benzer biçimde genel kullanım metninden başka VPN motorları desteği, belirli bir iş kuyruğu/veri tabanı teknolojisi, tam bir kez trafik muhasebesi veya kiracılar arası ortak sunucu paylaşımı çıkarmıyoruz. Site alt bilgisindeki genel VpnHood kaynak/audit bağlantıları da Hoodmin'in bağımsız olarak denetlendiği anlamına gelmez.

## Önerilen kapsam kararı

Pazarlama sayfasıyla tam eşdeğerlik hedeflendiğinde marka özelleştirmesi, davet, yönetim API'si ve belirtilen kurulum hedefleri de doğrudan kapsamın parçasıdır. Önceki tablodaki "eklense güzel olur" değerlendirmesi, dar BYOS başlangıç kapsamının önceliğiyle ilgilidir; bu özelliklerin Manager'da karşılığı olmadığı anlamına gelmez.

1. **Temel eşdeğerlik:** Çiftlik/yük dağıtımı, token URL, plan/profil ve hız/kota dönemleri, sertifika yönetimi/içe aktarma, ayrıntılı raporlar. Kurulum kolaylığı için resmî kurulum üzerinden otomasyon.
2. **Güvenilir dağıtım:** .deb ve kurulum adı geçişi, yedekleme/geri yükleme, anahtar döndürme, kesinti muhasebesi ve gerçek trafik/yük doğrulaması. Uzun işlemler eklenirken kalıcı iş altyapısı.
3. **Kapsam seçimi:** Bot yönetim API'si, çoklu çalışma alanı geçişi, platform yöneticisi/kayıt akışı, davet akışı, marka özelleştirmesi ve bulut IP otomasyonu kullanım ihtiyacına göre önceliklendirilmeli. Bot API'sinin Console'da doğrudan karşılığı olması, mevcut hedef için hemen zorunlu olduğu anlamına gelmez.

Hoodmin VpnHood'a odaklı kalır. Başka VPN sunucu yazılımlarını desteklemek bu karşılaştırmanın veya isim değişikliğinin parçası değildir.
