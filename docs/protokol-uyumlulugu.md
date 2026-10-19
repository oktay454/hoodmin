# VpnHood protokol başvurusu

İncelenen kaynak: `../VpnHood-Current`, Git `fec557a91908ffe27e238e7e6375a8ead997d2a4`. Hazır sunucu: **9.0.856**. Sunucu kurulumu resmî hazır dağıtımı kullanır; .NET projeleri sözleşme/istemci sınaması içindir.

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

## Sertifika ve adres yenileme

Sunucu başına HTTP-01 [AutoSign](autosign.md) vardır. Grup sertifika deposu ortak PFX'i mevcut/yeni/taşınan üyelere dağıtır; üyelikten çıkmak anahtarı geri çekmez. Grup yenileme URL'si `/f/{tenant}/{farm}/{seed}`, başlangıç sunucusunun sırrıyla şifrelenmiş uygun canlı hedefi döndürür. Yeni grup erişimli anahtarlarda kullanılır; eski anahtarın URL'si kendiliğinden değişmez. Hedefin kendi adresi, sertifikası ve sırrı korunur.

[Harici yayım](harici-adres-yayimi.md) GET okuma adresi ile PUT/POST/multipart yükleme adresini ayırır. Yayımın başarılı sayılması için GET içeriğinin gönderilen şifreli dosyayla aynı olması gerekir.

## Destek ve doğrulama sınırları

- Kaynak kataloğu 47 düzenlenebilir yaprak ayarı kapsar. Yerel çekirdek/yetkiler bazı ayarların etkisini sınırlar; sürüm onayı bütün işletim sistemi etkilerinin kanıtı değildir.
- Gerçek hazır sunucuda TCP/UDP/QUIC veri yolu, TUN/NAT, hız ve ağ filtresi, PFX ve HTTP-01 sınanmıştır. Masaüstü işletim sisteminin bütün rotaları ve uzun süreli yük/kesinti ayrı doğrulama gerektirir.
- Windows kurulum komutu üretilir; Windows üzerinde uçtan uca kurulum doğrulaması yoktur.
- İstemci politikalarının kapalı kaynak ticari ülke/plan kararları birebir çıkarılamaz. Hoodmin'in açık politika kuralları uygulanır; reklam ağı entegrasyonu yoktur.
- `UseUdpProxy2`, incelenen kaynak sürümünde uygulanabilir ayar olarak doğrulanmamıştır; desteklenen 47 ayara dahil değildir.
- Locked IPs, bulut IP satın alma ve markalama için bilinmeyen davranış uydurulmaz. Başka VPN motoru adaptörü bulunmaz.
- Kullanım raporlarının tekil rapor kimliği olmadığından kesintilerde tam bir kez (exactly-once) muhasebe garantisi verilmez.
- [Yönetim API'si](yonetim-api.md), Hoodmin'in kendi sözleşmesidir; kapalı kaynak Console'un özel API'sinin kopyası değildir.

Sınama araçları ve çalıştırma koşulları: [geliştirme ve doğrulama](dogrulama.md).
