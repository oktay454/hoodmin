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

## Önerilen kapsam kararı

1. **Temel eşdeğerlik:** Çiftlik/yük dağıtımı, token URL, plan/profil ve hız/kota dönemleri, sertifika yönetimi, ayrıntılı raporlar. Kurulum kolaylığı için resmî kurulum üzerinden otomasyon.
2. **Güvenilir dağıtım:** .deb ve kurulum adı geçişi, yedekleme/geri yükleme, anahtar döndürme, kesinti muhasebesi ve gerçek trafik/yük doğrulaması. Uzun işlemler eklenirken kalıcı iş altyapısı.
3. **Kapsam seçimi:** Bot yönetim API'si, çoklu çalışma alanı geçişi, platform yöneticisi/kayıt akışı ve bulut IP otomasyonu kullanım ihtiyacına göre önceliklendirilmeli. Bot API'sinin Console'da doğrudan karşılığı olması, mevcut hedef için hemen zorunlu olduğu anlamına gelmez.

Hoodmin VpnHood'a odaklı kalır. Başka VPN sunucu yazılımlarını desteklemek bu karşılaştırmanın veya isim değişikliğinin parçası değildir.
