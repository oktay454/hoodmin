# Hoodmin — canlı Console menü incelemesi

> Bu belge tarihsel inceleme kaydıdır. Son uygulama ve sınama durumu [protokol uyumluluğu](protokol-uyumlulugu.md) belgesindedir.

İnceleme tarihi: 16 Ekim 2026. Bu çalışma yalnız karşılaştırma ve belgelendirmedir; yeni işlev geliştirme onayı değildir.

## Kapsam ve kanıt düzeyi

Oturum açıkken mevcut Free projede, ekip ekranında Project Owner rolü görülen hesapla inceleme yapıldı. `Servers`, `Server Farms`, `Access Tokens`, `Access Token Profiles`, `Locked IPs`, `Settings → Project / Team / Server Profiles` menüleri ve ilgili ayrıntı/formlar incelendi. Kurulumun beş yolu bir önceki incelemede Deneme1 üzerinde ayrı ayrı doğrulanmıştı.

- **Menüden doğrulandı:** Görünür ekran, alan, seçenek veya açıklama bulundu. İşlevin uçtan uca çalıştığı anlamına gelmez.
- **Kaynakta doğrulandı:** Kullanıcının verdiği mevcut açık kaynak VpnHood kopyasında ilgili davranış okundu. Kapalı kaynak Console işleyicisiyle aynı olduğu varsayılmaz.
- **Bu hesapta doğrulanamadı:** Pazarlamada var, ancak erişilen menülerde karşılığı bulunamadı veya kayıt oluşturmadan ayrıntısına ulaşılamadı. Başka plan/sağlayıcı/hesapta bulunmadığı sonucu çıkarılmaz.

Hiçbir sunucu/anahtar/bot/kullanıcı/sertifika/IP kaydı oluşturulmadı veya silinmedi; kurulum/yeniden kurulum, PUSH ALL, sertifika imzalama/yükleme, rol değişikliği veya kimlik yenileme yapılmadı. Açılan formlar iptal edildi. AutoSign seçeneği yalnız açık formda görüntülenip CANCEL ile atıldı. Anahtar, JWT, yönetim sırrı ve özel anahtar değerleri bu belgeye alınmadı.

## Doğrudan maddelerin menü karşılıkları

| Madde | Menü / kanıt | Netleşen davranış veya sınır | Hoodmin karşılığı |
|---|---|---|---|
| Kendi sunucusunu getirme (BYOS) | Servers → ADD; mevcut sunucu ayrıntıları | Ekleme formu sunucu adıyla başlar; mevcut sunucu çiftliğe atanır. Sağlayıcıdan VPS satın alma formu gözlenmedi. | Kiracıya ait sunucu/grup kaydı mevcut. |
| Hazır Linux/Windows kurulumu | Server → INSTALL → beş seçenek | Parolalı SSH, anahtarlı SSH, Linux komutu, Windows komutu ve elle appsettings yolları. Önceki belgede ayrıntıları var. | Elle bağlantı JSON'u mevcut; resmî hazır kurucu için komut üretimi ve SSH yürütme eksik. Yeni VPN kurucusu yazmak gerekmiyor. |
| SSH ile bakım/güncelleme | Kurulu Server ayrıntısı | REINSTALL düğmesi görüldü; ayrı UPDATE/SSH bakım ekranı gözlenmedi. Kurulu sunucuya dokunmamak için REINSTALL çalıştırılmadı. | Bunu bağımsız ve doğrulanmış bir Console özelliği olarak zorunlu listelemiyoruz. Kurulum ile yeniden kurulum/güncelleme aynı şey sayılmamalı. |
| İstemciye erişim anahtarı verme | Access Tokens → mevcut anahtar → ACCESS KEY | `vh://` biçimi, kopyalama kontrolü ve DOWNLOAD düğmesi doğrulandı. Anahtar değeri kaydedilmedi, indirme yapılmadı. | Uyumlu `vh://` üretimi/kopyalama mevcut; uzak istemci bağlantısı önceki sınamalarda doğrulandı. |
| Tekli/toplu anahtar üretimi | Access Tokens → ADD | Profile, Name, Count, Group, Description alanları var. Anahtar ayrıntısında çiftlik ve profil ayrı seçilebilir. | Tekli/toplu üretim mevcut; profil ve çiftlik anahtarı modeli eksik. |
| Plan/profil | Access Token Profiles → mevcut profil | Private/Public ayrımı; Max Client, Max Cycle Traffic, Max Total Traffic, ayrı Download/Upload Speed, Lifetime, Ad Requirement, Policies. Availability oluşturulduktan sonra değiştirilemiyor açıklaması var. | Anahtar başına cihaz/toplam trafik/son tarih mevcut. Yeniden kullanılabilir profil, dönemsel kota ve ayrı yönlerde hız sınırı eksik. |
| Kota/süre birimleri | Profil → ilgili düzenleme düğmeleri | Trafik MB, hız Mbps, Lifetime gün; 0/boş sınırsız açıklaması var. Dönemsel kota alanı doğrulandı, sıfırlamanın takvimi veya saat dilimi formda gösterilmiyor. | Aylık sıfırlamayı yalnız pazarlama ifadesinden kesin bir uygulama kuralı olarak çıkarmamalıyız. Mevcut GiB arayüzüyle birim dönüşümleri ayrıca tasarlanmalı. |
| Çiftlik ve kapasiteye göre dağıtım | Server Farms; Server → Power | Çiftlik ve üyelik var. Power formu 1, 2, 4, 16 gibi göreli değerler ve boş bırakıldığında otomatik seçim açıklaması sunuyor. Sunucuda Allow Auto Select, Premium, Enabled alanları da var. | Yalnız gruplama var. Çiftlik anahtarı, göreli ağırlık ve seçim/yönlendirme eksik. Dağıtım algoritması veya aktif oturumların taşınması sınanmadı. |
| Anahtar bağlantısı / otomatik adres yenileme | Server Farm → Farm Token Urls | Publish Url, Name, Auto Upload; Off, POST, PUT, PUT & POST seçenekleri. Listede Status, Auto Publish, Last Pushed, Error; ayrıca PUSH ALL. Sonuçların birkaç dakika önbellekte kalabileceği açıklanıyor. Çiftlikte Push token to clients ve Allow Access Code alanları var. | Token URL deposu/yayımlama ve istemci yenileme eksik. Sadece HTTP GET adresi sunmaktan daha geniş bir yönetim akışı. PUSH çalıştırılmadı; yenileme davranışı uçtan uca sınanmadı. |
| Bulut sağlayıcıdan IP alma/bırakma | Servers → ADD, Server ayrıntısı/Access Points, Settings, Locked IPs incelendi | Bu hesapta sağlayıcı kimliği, yeni IP siparişi veya IP bırakma ekranı görülmedi. Locked IPs yalnız IP ve açıklama ekletiyor; bunun IP satın alma ekranı olduğu söylenemez. | Pazarlamada doğrudan karşılığı var, bu kurulumda menüden doğrulanamadı. Sağlayıcı entegrasyonu ayrı ve isteğe bağlı kapsam olarak kalmalı. |
| Erişim noktaları ve genel/özel adres ayrımı | Server → Access Points → ADD | IP, TCP/UDP/QUIC portları, Mode ve Listen. Mode seçenekleri Private, Public, PublicInToken. Automatic Configuration ve genel IPv4/IPv6 gösterimleri mevcut. | IP/FQDN ve TCP düzenleme var; bu ayrıntılı çoklu erişim noktası arayüzü yok. NAT'taki dinleme adresi ve anahtarda ilan edilen adres ayrımı korunmalı. |
| Sertifika oluşturma ve kullanıcının sertifikası | Farm/Server → SSL/TLS → ADD / UPLOAD | ADD kendinden imzalı sertifika üretimini anlatıyor; alan adı, kuruluş bilgileri, son tarih alanları var. UPLOAD yalnız PFX ve dışa aktarılabilir özel anahtar şartını, sertifika parolasını gösteriyor. | Sunucu başına kendinden imzalı sertifika var. Çiftlik sertifikası yönetimi ve kullanıcı PFX içe aktarma arayüzü eksik. |
| Otomatik sertifika imzalama/yenileme | Mevcut sertifika → Signing | Manual, AutoSign, DomainOnly seçenekleri doğrulandı. Ayrıntıda Visibility, Used By, Priority, imza ve son tarih bulunuyor. | Otomasyon yok. Let's Encrypt pazarlamada belirtiliyor; bu incelemede imzalama, ACME doğrulaması veya otomatik yenileme çalıştırılmadı. Menü etiketi ile çalışan yenileme garantisi ayrılmalı. |
| Canlı durum ve geçmiş grafikler | Overview; Server → Usage | Overview'de Sessions, Transfer Speed, Active/Idle/Lost Servers başlıkları var; bu incelemede genel sayaçlar `-` gösterdi. Sunucu ayrıntısında canlı oturum/hız/CPU/bellek ve sürüm/OS; Usage altında Sessions, Transfer Speed (Mbps), CPU (%), Available Memory (GB) grafikleri görüldü. | Canlı temel durum ve toplam sayaçlar mevcut; hız ve geçmiş zaman serileri eksik. Console'da ayrı kişi bazlı canlı oturum yönetim ekranı bu incelemede görülmedi. |
| Rapor dönemleri ve ülke kırılımı | Usage Date Period; Countries → View | Son saat, bugün/dün, 24/48/72 saat, 7/31 gün, mevcut ay. Countries Usage ekranında sunucu filtresi ve Country / Used Devices sütunları var; sınama sunucusunda satır yoktu. | Tarih filtreli geçmiş raporları/ülke kırılımı eksik. Ülke başına bayt raporu bulunduğu iddia edilmemeli; doğrulanan ölçü cihaz sayısı. |
| Ekip ekleme ve roller | Settings → Team → ADD USER; mevcut üye → Role | ADD USER yalnız Email istiyor. Roller: Project Reader, AccessTokenWriter, Project Writer, Project Admin, Project Owner. | Yerel hesap oluşturma ve üç rol mevcut. Beş rolle yetki eşdeğerliği ayrıca ele alınmalı. E-posta gönderme, davet kabulü veya mevcut Google hesabına doğrudan üyelik verme davranışı kayıt yapmadan doğrulanmadı. |
| Bot ve yönetim API anahtarı | Settings → Team → ADD BOT | İsim isteyen bot oluşturma formu görüldü. Projede mevcut bot yok; yeni bot oluşturmadan API anahtarı ekranı/yetki kapsamına ulaşılamadı. | Bot kimlikleri/yönetim API anahtarları eksik. Bot menüsü doğrulandı; bütün ekranların REST API'den aynı yetkilerle yönetilebilmesi hâlâ pazarlama iddiası düzeyinde, çağrılar sınanmadı. |
| Kendi markasıyla arayüz sunma | Settings → Project/Team/Server Profiles, çiftlik/sunucu/profil alanları | Erişilen menülerde logo, ürün adı veya tema markalama ayarı görülmedi. Project ekranında ad, plan, Google Analytics ve reklam ödül alanları var; bunlar markalama ekranı sayılmaz. | Hoodmin markası sabit. Manager'daki yönetilebilir markalama pazarlamada var, bu Free hesapta menüden doğrulanamadı. Kiracı başına markalama veya özel alan adı kapsamı varsayılmamalı. |

## Bakım/güncelleme için kaynak kodunun söylediği

Kullanıcının "muhtemelen yine aynı kurucuyu kullanıyor" değerlendirmesi, resmî güncelleyicinin yaklaşımıyla uyumludur:

- Mevcut `VpnHood-Current/pub/lib/vh-installer/linux/updater.sh`, yayımlanan sürüm ve UpdateCode bilgisini karşılaştırır; yeni kurucuyu indirip `-q -destination ...` ile çalıştırır. Kapsayıcı kipinde `-container` da eklenir.
- Windows `pub/lib/vh-installer/win/updater.ps1` de yeni kurucu üzerinden sessiz güncelleme yaklaşımını içerir.
- Bunlar açık kaynak **sunucu kurulum/güncelleme araçlarının** davranışıdır. Console'un REINSTALL düğmesinin veya başka bir işleyicisinin SSH üzerinden bunları birebir çağırdığı doğrulanmadı.

Bu nedenle "SSH bakım/güncelleme sistemi yeniden yazılacak" ifadesi yerine, ihtiyaç olduğunda resmî kurucu/güncelleyiciyi kullanan yönetim akışı değerlendirilmelidir. Bağımsız bakım otomasyonu, Console'da doğrulanmış doğrudan gereksinim olarak gösterilmez.

## Pazarlama listesinden ayrı, menüde bulunan ek farklar

- **Server Profiles:** Normal/premium oturum sınırı; IPv6, yerel ağ ve IP aralığı filtreleri; ICMP/TCP/UDP ve istemci/hedef bilgilerini izleme seçenekleri.
- **Advanced Edit:** Tracking, Session, NetFilter; TCP/UDP/QUIC uç noktaları, DNS, zaman aşımı/kanal sınırları, swap, TCP congestion control ve HTTP-01 gibi JSON alanları. Yalnız alan adları okundu, değerler kaydedilmedi.
- **Access Token Profiles → Policies:** Ülke/konum, deneme/reklam/premium davranışına ilişkin örnek politikalar içeren JSON düzenleyicisi. Bunların örnek metni, mevcut projede etkin politika olduğunun kanıtı değildir.
- **Access Tokens:** Destek kodu, profil/grup filtreleri, çiftlik/profil atama, dönemsel/toplam trafik ve bitiş tarihi; müşteri/sipariş kimliği alanları. Bu alanlar ödeme/faturalama sisteminin varlığını kanıtlamaz.
- **Farm:** Sunucu profili seçimi, istemcilere token gönderimi ve access code seçeneği.
- **Locked IPs:** IP, kilitlenme zamanı ve açıklama kaydı. İşlevinin kapsamı adından çıkarılmadı, kayıt eklenmedi.

## Sonuç ve uygulama kapsamı

Menüden doğrulanan doğrudan farklar artık komut üretimi/SSH kurulum, plan ve ayrı yönlerde hız, çiftlik/Power, token URL yayımlama, PFX/sertifika imzalama, raporlar, ekip/rol ve bot oluşturma olarak somutlaştırıldı. Markalama, bulut IP siparişi ve bot API sözleşmesinin ayrıntıları için kanıt sınırı korunur.

Bu inceleme bir işlev sınaması değildir. Gerçek dağıtım algoritması, anahtar yenilemesi, imza/yenileme, davet kabulü, bot yetkileri, dönemsel sıfırlama takvimi ve sağlayıcı IP işlemleri kayıt değişikliği yapılmadan doğrulanmış sayılmaz. Kullanıcı onayı olmadan bu alanlarda yeni geliştirme başlatılmadı.
