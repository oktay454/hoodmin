# Duraklama kaydı — 17 Ekim 2026

Kullanıcının isteği: eldeki işi tamamlayıp durakla; kullanıcı “devam et” demeden sonraki işe başlama.

## Tamamlanan son iş

- Grup adresi yenileme URL'si: ulaşılamayan başlangıç adresinden uygun canlı grup üyesine geçiş. Gerçek .NET belirteç çözme ve hazır VpnHood 9.0.856 istemci bağlantısı sınandı.
- Şifreli ortak grup PFX deposu; mevcut, yeni ve gruba taşınan üyeye dağıtım. VPN adresi ve her sunucunun kendi sırrı korunur. Açık onay ve sürüm denetimi vardır.
- Tek sunucu PFX değişiminde DNS adresinin korunması ve aynı X509 DNS adına sahip sertifikaların çakışmasının önlenmesi.
- Türkçe, Arapça ve İngilizce çeviriler, üç benioku ve yönetim API belgesi güncellendi.

Ayrıntılı kanıtlar: [doğrulama kaydı](dogrulama.md), [kaynak sözleşmesi ve kapsam](protokol-uyumlulugu.md).

## Devam izninden sonraki işler

1. Console'da görülen harici POST/PUT şifreli adres yayımı. Henüz kodlanmadı.
2. Grup düzeyinde sunucu profili atama ve değişiklikleri üyeye otomatik yayma. Mevcut seçili sunuculara açık toplu uygulama çalışır.
3. Otomatik CA/ACME sertifikası alma ve yenileme. Kullanıcı bunun sınaması için ayrı sunucu sağlayacağını bildirdi; henüz bağlantı bilgileri gelmedi. Mevcut süreli HTTP-01 yanıtı otomatik CA hizmeti değildir.
4. Debian paketi, üretim yedekleme/geri yükleme ve ana anahtar döndürme; uzun süreli yük, kesinti, UDP kullanıcı verisi ve işletim sistemi VPN yönlendirmesi sınamaları.

Kapalı kaynak Console ile tam eşdeğerlik iddia edilmez. Ülke bazlı ticari istemci kararları, Locked IPs çalışma etkisi, IP siparişi ve marka özelleştirmesinin gözlenmeyen davranışları açık kapsam kaydındadır.

## Çalışan sınama ortamı

- Panel: https://vhm-test.oktayaktogan.com.tr/ ; `yapayzeka` kapsayıcısı, `vpnhood-manager` hizmeti.
- VPN: `vpnhood` kapsayıcısında resmî hazır `/opt/VpnHoodServer`, `VpnHoodServer.service`, TCP2550 / UDP2551 / QUIC2552. Kaynak sunucu uygulaması yeniden derlenmedi.
- Kullanıcının eski `vpnhood-test` kaydına dokunulmadı. Bu kayıt daha önce kimlik yenilendiği için çevrimdışıdır.
- Özel sınama kimlik bilgileri/sertifikaları ve önceki yapılandırma yedekleri `work/` içindedir; Git dışında kalır. Sınama HTTP80 ve özel ağ erişimi iş sonunda kapatılır.
- 0008 şema değişikliği uygulandı. Uygulanmış geçişi geriye dönük değiştirmeyin; sonraki değişiklik için yeni geçiş kullanın.

Bu sonlandırma sırasında yeni sistem paketi kurulmadı. Önceki doğrulama aşamasında iki kapsayıcıya Microsoft deposundan `libmsquic 2.6.3`, Debian'dan `libnuma1`; `vpnhood` kapsayıcısına ayrıca Debian'dan `sudo` kuruldu.
