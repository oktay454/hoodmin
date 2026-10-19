# VpnHood yönetim sözleşmesi ve uyumluluk çalışması

İncelenen kaynak: `../VpnHood-Current`, Git `fec557a91908ffe27e238e7e6375a8ead997d2a4`. Hazır sunucu: **9.0.856**. VpnHood sunucusu bu çalışma için yeniden derlenmedi; .NET projeleri yalnız sözleşme/istemci sınaması içindir.

Makinece okunabilir alan kataloğu: `crates/protocol/contract-catalog.json`. Katalog kaynak dosyalarının SHA-256 özetlerini içerir; kurulum dosyalarını veya sırları okumaz. Yenileme:

```sh
python3 scripts/catalog-contract.py ../VpnHood-Current crates/protocol/contract-catalog.json
```

## Sunucunun aldığı komutlar

`HttpAccessManager`, verilen kök adresin `/api/agent/` yolunu kullanır. Kimlik `Authorization: Bearer …` başlığındadır.

| Yol | Yöntem | Girdi | Yanıt / etkisi |
|---|---|---|---|
| `configure` | POST | `ServerInfo` | `ServerConfig`; sunucu `storage/internal/last-config.json` önbelleğine yazar |
| `status` | POST | `ServerStatus`, kullanım farkları | `ServerCommand`: `configCode` ve oturum kararları |
| `sessions` | POST | `SessionRequestEx` | `SessionResponseEx`: erişim, yönlendirme, hız, kota, süre, reklam kararı |
| `sessions/{id}` | GET | oturum, hostEndPoint, clientIp | Mevcut oturumun kurtarılabilen anahtarı ve güncel kararı |
| `sessions` | GET | — | İlgili sunucunun oturumları |
| `sessions/{id}/usage` | POST | trafik farkı; `adData`, `closeSession` sorgu alanları | `SessionResponse` |
| `sessions/usages` | POST | `SessionUsage[]` | Oturum kimliğine göre yanıt sözlüğü |
| `acme/http01_key_authorization` | GET | `token` | JSON metin olarak keyAuthorization; bulunamazsa 404 |

**Genel amaçlı kabuk/SSH komutu agent sözleşmesinde yoktur.** Kurulum/yeniden kurulum ayrı SSH yürütücüsünün işidir. Sunucu durum yanıtındaki configCode değişince configure isteğini yeniden yapar. Oturum kararları kapanış bildirimi gelene kadar tekrar iletilir.

## ServerConfig alanları

53 sayısı grup başlıkları ve üç korunan alanı da kapsar. Doğrudan düzenlenebilir **47 yaprak ayar** vardır. Grup adlarının JSON yazımı büyük harflidir: `Session`, `Tracking`, `NetFilter`; alt alanlar HTTP sözleşmesinde camelCase kullanır.

| Grup | Alanlar |
|---|---|
| Kök | tcpEndPoints, udpEndPoints, quicEndPoints, dnsServers, updateStatusInterval, logAnonymizer, minCompletionPortThreads, maxCompletionPortThreads, swapMemorySizeMb, addListenerIpsToNetwork, tcpCongestionControl, enableHttp01Challenge |
| Session | timeout, udpTimeout, tcpTimeout, icmpTimeout, syncInterval, syncCacheSize, maxPacketChannelCount, maxUdpClientCount, maxIcmpClientCount, tcpKernelBufferSize, streamProxyBufferSize, udpProxyBufferSize, udpChannelBufferSize, tcpConnectTimeout, channelIdleTimeout, maxTcpConnectWaitCount, maxTcpChannelCount, netScanLimit, netScanTimeout, allowTcpProxy, allowTcpPacket |
| Tracking | trackClientIp, trackLocalPort, trackDestinationIp, trackDestinationPort, trackUdp, trackTcp, trackIcmp |
| NetFilter | networkIsolation, includeLocalNetwork, vpnAdapterIncludeIpRanges, vpnAdapterExcludeIpRanges, includeIpRanges, excludeIpRanges, blockIpV6 |
| Korunan | serverSecret: otomatik üretilen sunucu kimliği; configCode: sürümleme; certificates: özel anahtar içeren PFX listesi |

Türler ve kaynak dosyaları katalogdadır. Dinleyiciler IP:port; IP aralıkları tek IP, CIDR veya başlangıç-bitiş; tamponlar `{ "send": bayt, "receive": bayt }`; süreler .NET TimeSpan biçimindedir. Dönem aralığı için yanlışlıkla sessiz varsayım yapılmaz.

Gelişmiş ayarlar ekranı kaynakta bulunmayan alanları reddeder, sunucu sırrını ve PFX verisini göstermez, eski açık formun yeni ayarları ezmesini engeller. Sunucunun configCode onayı ve dinleyici sonuçları izlenir. Yerel appsettings.json içindeki ServerConfig, merkezi ayarlara üstün gelebilir. Çekirdek (kernel) ve LXC yetkileri nedeniyle swap, arayüze IP ekleme ve TCP congestion control her ortamda etkili olmayabilir; configCode onayı bu işlemlerin çekirdek düzeyinde başarılı olduğunun tek başına kanıtı değildir.

## Oturum ve erişim kararı

- İstemci kanıtı .NET GUID little-endian + AES-CBC biçiminde doğrulanır.
- İstek sözleşmesinin ülke, plan, konum, yönlendirme izni, IPv6, erişim kodu, ek veri, kullanıcı değerlendirmesi ve anonim ölçüm izni alanları tanımlıdır.
- Hız, `AccessInfo.MaxSpeedMbps` alanından uygulanır: `sent` istemcinin yüklemesi, `received` indirmesidir. Sunucu kendi gönderme/alma yönünde bunları ters kullanır.
- Toplam trafik ve dönemlik trafik ayrı sayaçlardır. Dönem uzunluğu `cycleSeconds` ile açıkça seçilir; Console'un ticari faturalama takvimi bilinmediği için aynı olduğu varsayılmaz.
- İlk başarılı kullanımdan başlayan ömür, kesin son tarih, eşzamanlı cihaz sınırı ve cihaz kayıt ömrü ayrıdır.
- Aynı cihazın önceki oturumu `SessionSuppressedBy/YourSelf` kararı alır. İsteğe bağlı diğer cihaz bastırması `Other` olarak bildirilir.
- `farmAccess` açık anahtarlar yalnız kayıtlı oldukları kiracı ve grup üyelerinde kullanılabilir. Genel erişim anahtarları kendiliğinden kiracılar arasında paylaşılmaz.
- Yönlendirmede `RedirectServerTokens` kullanılır. Hedefin kendi sertifika parmak izi ile adresi iletilir; yönlendirme döngüsü engellenir. Güncel protokol 10+ hedeflenir; daha eski istemci için farklı sertifikalı grup yönlendirmesi UnsupportedClient döndürür.
- Bölge seçimi, göreli sunucu gücü (0=işlemci sayısı), normal/premium kapasitesi ve etkinlik yönetici karar katmanındadır.
- Erişim kodları rastgele oluşturulur, yalnız özetleri saklanır; başka kiracının veya izin verilmeyen sunucunun anahtarını açmaz.
- Ödüllü reklam, istemcinin AdData metnine güvenilerek kabul edilmez. Güvenilir doğrulayıcı sunucunun kimlikli bildirimi gerekir. Aynı bildirim ikinci ödül vermez. Bu, Hoodmin'in SSV entegrasyonudur; Console'un kapalı kaynak callback API'sinin birebir kopyası olduğu iddia edilmez.
- Google ölçüm kimliği yalnız istemcinin anonim ölçüm izni varsa iletilebilir. API sırrı agent sözleşmesinin alanı değildir.

## Adres ve sertifika

ServerToken alanları hname, hport, isv, ep, ep_st, path, urls, loc2 ile yönetilir. sec/ch/ct doğrudan duyuru düzenleyicisinde değiştirilmez. Sunucu adresi yayımı .NET `ServerToken.Encrypt/Decrypt` ile uyumlu AES-CBC/PKCS7 ve `Base64(IV).Base64(ciphertext)` biçimindedir.

PFX yükleme özel anahtarın varlığını, sertifika-anahtar eşleşmesini ve geçerlilik tarihlerini denetler. Sertifika listesi şifreli saklanır. Varsayılan sertifika değişimi açık onay gerektirir; eski anahtarlardaki parmak izi adres yayımıyla veya anahtarın yeniden paylaşılmasıyla güncellenmelidir. HTTP-01 doğrulama yanıtı 15 dakika geçerlidir. Otomatik CA sertifikası üretimi/yenilemesi bunun üzerinde ayrı bir süreçtir; gerçek alan adı ve dış TCP 80 yolu ayrıca sınanmalıdır.

## Kurulum

Linux ve Windows için resmi hazır kurulum komutları, appsettings.json ve parola/özel anahtarlı SSH yolu sunulur. SSH sunucu anahtarı SHA-256 parmak iziyle sabitlenir; uyuşmazlıkta bağlanılmaz. SSH parola/özel anahtarı veritabanına kaydedilmez; özel çalışma zamanı dizininden iş sonunda silinir. Agent kimliği sonraki kurulum için AES-GCM ile saklanır. Eski kayıtlardaki yalnız özetten kimlik geri üretilemez; mevcut kimliğin girilmesi gerekir.

Kurulum işi önceki appsettings dosyasını hedefte 0600 izinli yedekler. Aynı sunucuya eşzamanlı ikinci kurulum açılmaz. Çalışan SSH işinin zaman aşımı vardır. Paket bağımlılıkları Debian önceliğiyle kurulur; resmi VpnHood betiği kendi güncelleme/bağımlılık akışını yönetir.

## Doğrulama kaydı

- [x] Mevcut HTTPS/PostgreSQL/.NET panel sınamaları yeni protokol katmanında geçti.
- [x] 47 ayarın HTTP yanıtına kayıpsız taşınması ve gerçek .NET ServerConfig ile çözümlenmesi geçti.
- [x] Hız/kota/ömrün gerçek SessionResponseEx ile çözümlenmesi geçti.
- [x] Şifreli adres yayımı gerçek .NET ServerToken.Decrypt ile geçti.
- [x] Dönem kotası dolması, dönem değişimi ve toplam sayacın korunması geçti.
- [x] Grup yönlendirmesi, konum, grup dışı erişim engeli, ortak kullanım sayacı ve kapasite sınırı geçti.
- [x] Erişim kodu, doğrulanmış ödül ve tekrar bildiriminin ikinci ödül vermemesi geçti.
- [x] Hazır 9.0.856 sınama sunucusu TCP/UDP, DNS ve oturum ayarlarını aldı; uygulanan önbellekte doğrulandı.
- [x] SSH özel anahtar ve parola+sudo ile resmi hazır kurulum gerçek sunucuda geçti; yanlış sunucu parmak izi bağlantıdan önce reddedildi.
- [x] PFX/HTTP-01 ve QUIC hazır sunucuda doğrulandı.
- [x] Gerçek TCP indirme hızı ve ağ filtresi sınamaları geçti.
- [ ] Console menüleriyle son karşılaştırma ve eksik ticari yönetici işlevlerinin kapsam kontrolü sürüyor.

Console'da görünen UseUdpProxy2, bu kaynakta ve hazır sunucunun son uygulanan önbelleğinde yoktur. Bu, Console'un HTTP yanıtında hiç bulunmadığı anlamına gelmez; sunucuda uygulanabilir bir alan olduğu teyit edilmemiştir. Default profilin gelişmiş düzenleyicisinde deneme değeri kalıcılaşmadı; bu deneme doğrulanmış özellik olarak sayılmadı. Açık taslak iptal edildi; başlangıç yapılandırması özel sınama kopyasında tutuldu.

## Bu çalışma sırasında kapsayıcıya eklenen paketler

`vpnhood`: Microsoft deposundan libmsquic 2.6.3; Debian deposundan libnuma1 ve sudo. `yapayzeka`: Microsoft deposundan libmsquic 2.6.3; Debian deposundan libnuma1. SSH sınamasının geçici hesabı, sudo yetkisi, yetkili anahtarı ve ayrı SSH dinleyicisi kaldırıldı; ana SSH hizmeti değiştirilmedi.

## Güncel uygulama matrisi — 17 Ekim 2026

Önceki Console inceleme belgeleri inceleme tarihindeki durumu korur. Aşağıdaki tablo yeni geliştirmelerin durumudur; menüde görünmek, tel sözleşmesiyle çözümlenmek ve gerçek sunucuda uygulanmak ayrı kanıtlardır.

| Konu | Hoodmin uygulaması | Doğrulama |
|---|---|---|
| 47 gelişmiş sunucu ayarı | Tür/aralık denetimi, sürüm çakışması koruması ve uygulama durumu | Tam .NET tür karşılaştırması; gerçek hazır sunucuda TCP/UDP/QUIC, DNS, oturum ve ağ ayarları |
| Linux SSH kurulumu | Anahtar veya parola; root/sudo, sabit SSH parmak izi, geçici sır temizliği | İki gerçek kurulum; yanlış parmak izi olumsuz sınaması |
| Elle Linux/Windows kurulumu | Resmî kurucu komutu ve bağlantı JSON'u | Parametreler kaynakla karşılaştırıldı; Linux gerçek kurulum geçti; Windows çalıştırılmadı |
| Erişim profilleri | Hız, kota/dönem, ömür, cihazlar, deneme/reklam politikası | HTTPS/PostgreSQL/.NET; dönem sıfırlama ve toplamın korunması |
| Sunucu profilleri | Ayar ve kapasite şablonu; seçili sunuculara tek işlemde uygulama | Çapraz kiracı toplu işlem bütünüyle reddedildi; kimlik/PFX korundu; eski sürüm 409 |
| Farm/Power/Location | Aynı kiracı grubunda uygun konuma ve göreli yüke yönlendirme | Grup dışı erişim, kapasite, farklı hedef sertifikası ve ortak kota sınamaları |
| Token URL | Sunucu ve grup HTTPS GET üzerinden .NET uyumlu AES-CBC şifreli ServerToken | Ulaşılamayan başlangıç adresinden canlı üyeye yenileme ve gerçek bağlantı; eski anahtara URL sonradan kendiliğinden eklenmez |
| Grup PFX deposu | Şifreli ortak depo; mevcut/yeni/taşınan üyelere dağıtım | Yanlış parola 400, eski sürüm 409; VPN adresi/sır koruma; gerçek TLS ve istemci bağlantısı |
| PFX | Parolalı dosya denetimi, şifreli saklama, açık varsayılan onayı | Yanlış parola/eskimiş form reddi; hazır sunucunun TLS bağlantısında yeni DER sertifikası doğrulandı |
| HTTP-01 | Kiracı/sunucu kapsamlı, süreli yanıt | Hazır VPN sunucusunun HTTP80 yolu gerçek yanıt verdi; bilinmeyen token 404 |
| Gerçek trafik | TCP proxy ve QUIC stream | TCP 12 MiB veri doğrulandı; 10 MiB ısınmadan sonraki 2 MiB, 1 Mbps sınırında 16,72 sn; QUIC 256 KiB |
| Ağ filtresi | Include/exclude aralıkları ve özel ağ erişimi | Önce erişilen aynı hedef, kural değişince HTTP401 ile reddedildi; hedefte yeni bağlantı olmadığı doğrulandı |
| Grafikler ve ülkeler | 1 saat–31 gün; sunucu filtresi; CPU/bellek/oturum/hız; ülke/cihaz sayısı | Gerçek agent durum örnekleri, birim dönüşümleri, salt okunur erişim ve çapraz kiracı engeli |
| Anahtar bilgileri | Ad, grup, açıklama, müşteri/sipariş/destek kodu; dosya indirme | Güncel anahtar dışa aktarımı ve yetki/HTML kaçış sınamaları |
| Ödüllü reklam | Güvenilir SSV bildirimi + bir kez tüketilen kısa ömürlü kayıt | Olumlu/olumsuz/tekrar bildirimi sınandı; reklam ağı bağlanmadı |

Çevrimiçi durumu yapılandırılmış bildirim aralığının üç katını kullanır; alt sınır 90 saniyedir. Rapor çizgileri ölçüm ortalamasıdır; ayrı sunucuların toplamını veya boş aralıkların sıfır kullanımını gösterdiği iddia edilmez. Ülke, agent isteğinde bildirilen değerdir; ayrıca IP coğrafi konumlandırması yapılmaz.

### Açık kalan işler

- Gerçek DNS/TCP80 sunucusunda Certbot kancalarıyla CA alma/yenileme ve PFX dağıtımı doğrulandı. Çok kiracılı panelden kendi ACME işini oluşturma/yönetme ekranı halen eksiktir.
- Console'un harici POST/PUT token yayımı ve grup düzeyinde sunucu profili atama/otomatik yayma.
- Beş ekip rolü ve bağımsız bot yönetim API'si son aşamada eklendi; Console'un özel yönetim API sözleşmesiyle birebir aynı olduğu iddia edilmez.
- İstemci politikalarının (`cpols`) ülkeye göre ticari karar kuralları. Kaynak, bu kararların kapalı kaynak sunucu tarafındaki anlamını bütünüyle tanımlamaz; root politika süreleri Hoodmin'de açıkça seçilir.
- Locked IPs kaydının çalışma etkisi, bulut IP siparişi ve markalama bu kaynak sözleşmesinden çıkarılamaz; gözlenmeyen davranış uydurulmadı.
- UDP kullanıcı verisi yeni genel sunucuda TCP/UDP/QUIC paket kanalları ve sunucu TUN/NAT üzerinden doğrulandı. Masaüstü işletim sistemi rotaları, uzun kesinti ve sürekli yük halen ayrı sınama ister.
- Debian paketi ve ayrı veritabanına yedek kurtarma tamamlandı. Ana anahtar döndürme, harici alarm kanalı ve uzun süreli işletim sınamaları ayrıca tamamlanacak.

Sınama için yalnız ayrılmış kapsayıcının `/opt/VpnHoodServer` kurulumu değiştirildi; kullanıcının eski `vpnhood-test` hizmeti ve kaynak ağacındaki sunucu uygulaması yeniden derlenmedi. Yeni sertifika yalnız bu sınama kaydındadır. Geçici SSH hesabı/anahtarı, özel ağ erişimi ve HTTP80 dinleyicisi sınama sonunda kapatıldı.

## Ekip ve bot API doğrulaması — 17 Ekim 2026

Beş rol eklendi; mevcut hesaplar kendiliğinden yükseltilmedi. Sahip ataması yalnız sahip tarafından yapılabilir; son etkin sahip ve yönetici korunur. Anahtar yazarı sunucu ayarı değiştiremez. Botlar rol ve son tarihle oluşturulur; anahtar özeti saklanır, parola girişi kapalıdır, insan üye yolundan sahip rolü alamazlar.

`tests/test_team_bots.py` gerçek HTTPS/PostgreSQL ile olumlu ve olumsuz yetki sınamalarını geçti: eski ayar 409, bozuk/geçersiz/iptal edilmiş/süresi dolmuş bot 401, başka kiracının botu 404, salt okunur sır indirme ve yetki yükseltme 403. Bot anahtarıyla toplu VPN anahtarı ve sunucu oluşturma, JSON ayarı kaydetme doğrulandı. Panel, sözleşme, farm/ödül, profil/rapor ve tema/dil sınamaları bu değişiklikten sonra yeniden geçti. API girişleri panelin aynı işlevlerini kullanır; alanlar ve durum kodları [API belgesinde](yonetim-api.md) listelenmiştir.

## İlk sunucuya ulaşılamadığında grup adresini yenileme

Kaynak `VpnProfileService.UpdateServerTokenByUrl`, eski ServerToken.Secret ile yeni hedef belirtecini çözer; `IsTokenUpdated` denetiminden sonra profilini günceller. `VpnHoodApp`, bağlantı hatasında bu yolu deneyip yeniden bağlanır. Bu nedenle yalnız çalışan ilk sunucunun RedirectHost yanıtını uygulamak, ilk adrese ulaşılamayan durumu kapsamıyordu.

Hoodmin'e ayrı grup URL yolu eklendi: `/f/{tenant}/{farm}/{seed}`. Yalnız yayımı açık grupların grup erişimli yeni anahtarlarında kullanılır. Yanıt, başlangıç sunucusunun sırrıyla şifrelenir; içerik uygun canlı hedefin kendi adresi, sertifika parmak izi ve sırrıdır. Hedefin sonraki yenileme URL'si de hedef sırrına göre değişir. Tek sunucu anahtarları kendiliğinden başka sunucuya yönlendirilmez. Kimlik, sertifika veya kiracı ortaklaştırılmaz.

`tests/test_farm_publication.py` geçti: kullanılmayan TCP2999 başlangıç adresine erişilemediği doğrulandı; gerçek hazır TCP2550 hedefi aynı kiracının geçici grubundayken .NET eski sırla hedefi çözdü, hedefin kendi sertifika/sır/adresini karşılaştırdı ve yenilenen anahtarla gerçek VpnHood istemcisi bağlandı. Hedefin önceki grup üyeliği geri yüklendi. Başlangıç hizmetinin önce çalışıp sonra çöktüğü veya tam mobil uygulamanın profil deposu/UI akışının çalıştırıldığı iddia edilmez; gerçek bağlantı ve kaynak yenileme sözleşmesi sınandı.

## Ortak grup PFX deposu — 17 Ekim 2026

PFX özel anahtarı kiracı/grup bağlamına bağlı AES-256-GCM ile saklanır. Açık onayla mevcut üyelere tek veritabanı işlemi içinde dağıtılır; yeni kaydedilen ve gruba taşınan sunucular da sertifikayı devralır. VPN bağlantı adresi ve her sunucunun kendi sırrı korunur. Sürüm denetimi eski formları reddeder. Grup üyeliğini kaldırmak, daha önce dağıtılmış özel anahtarı o sunucudan kendiliğinden geri çekmez.

`tests/test_farm_certificates.py` geçti: yanlış parola 400, eski form 409; mevcut/yeni/taşınan üyede PFX, sunucu sırrı ve DNS adresi korundu. Hazır 9.0.856 sunucusu eski bağlantı adı/SNI ile yeni sertifikayı sundu; DER ve erişim anahtarı parmak izi karşılaştırıldı, gerçek istemci bağlandı. Aynı X509 DNS adı farklı arayüz etiketiyle yenilenince yerel TLS seçicisinde mükerrer kayıt oluşmadı. Sınama hedefinin önceki şifreli yapılandırması ve grup üyeliği geri yüklendi. Otomatik CA hizmeti ve harici POST/PUT yayımı bu çalışmanın parçası değildir.

## Yeni genel sunucular — 18 Ekim 2026

Ayrılmış `hoodmin` ve `vh01` kapsayıcıları, resmî hazır 9.0.856 ve gerçek Let’s Encrypt alan adlarıyla kuruldu. CA alma ve yenileme `hoodmin-acme` Certbot kancaları üzerinden Hoodmin yönetim API'sini kullanır. Gerçek ilk dağıtımda PFX/TLS eşleşmesi ve CA doğrulaması geçti; yenileme `--dry-run` ile geçmiştir. Bu yol root işletmen yapılandırmasıdır; Console AutoSign ekranıyla aynı kullanıcı akışı olduğu iddia edilmez.

Gerçek DNS istemcisi bir hatayı ortaya çıkardı: PFX değişimi `isv` değerini false yapıyordu. Bu değer, kaynaktaki `EndPointResolver` için DNS çözümlemesini de denetler; adres listesi olmayan DNS anahtarı çalışmıyordu. Sertifika değişimi artık DNS/IP çözümleme bayrağını korur; pin yeni sertifikaya güncellenir. İki Rust gerileme sınaması ve grup PFX denetimi eklendi.

`tests/UdpProbe` resmî .NET istemci kitaplığı ve MsQuic soket fabrikasıyla TCP, UDP ve QUIC paket kanallarını ayrı ayrı seçti; geri düşmeyi reddetti. Atanmış sanal istemci IP'sinden yapılan UDP DNS isteği gerçek yanıt verdi. Sunucu TUN/NAT sayaçlarının arttığı ve kapsayıcı yeniden başlatıldıktan sonra QUIC verisinin tekrar geçtiği görüldü. NullVpnAdapter'a sentetik paket verildiğinden masaüstü işletim sisteminin tüm yönlendirme tablosu sınanmış sayılmaz.

Debian paket kurulum/yükseltmesi, geçiş özetleri, şifreli ayrı makine yedeği ve ayrı veritabanına ana anahtarla geri yükleme doğrulandı. [Ortam, işletim sınırları ve ayrı onay bekleyen UID/GID taşıması](uretim-ortami.md) kaydındadır.

## Console harici yayım davranışı — 19 Ekim 2026

[Açık onaylı QA gözlemi](console-harici-yayim-sinamasi.md): PUT başarısızsa POST'a geçiş, PUT başarılıysa POST'un atlanması, JSON gövde şablonu ve başlık yer tutucusu doğrulandı; Console OK durumu alındı. URL sorgusundaki yer tutucu değiştirilmeden geldi; Form-Data henüz doğrulanmadı. Bu gözlem Hoodmin'de harici yayıncı uygulaması veya tam eşdeğerlik iddiası değildir.

Kullanıcı ModSecurity'yi sınama FQDN'inde kapattıktan sonra ham şifreli Body ve multipart Form-Data tekrar sınandı. İki akışta da Console OK alındı; form alanlarında `{farm_token}` ve `{access_token}` değişimi doğrulandı. JSON sarmalaması gerekmedi. İlk aşamadaki Form-Data belirsizliği giderildi; URL sorgusu yer tutucusu sorunu ve uzun süreli otomatik yayım sınırları korunur.
