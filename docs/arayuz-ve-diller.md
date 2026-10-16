# Arayüz, yardım ve diller — 16 Ekim 2026

Kullanıcının isteğiyle füme tema, belirgin kart/tablo sınırları, simgeli gezinme, görünür Hesabım bağlantısı ve Türkçe/İngilizce/Arapça dil desteği uygulandı. Yeni paket veya bağımlılık kurulmadı.

## Dil ve yerleşim

- Türkçe varsayılandır. Dil, giriş ve panel ekranlarından değişir; tarayıcı tercih çereziyle bir yıl saklanır. Çerez yalnız dil kodu taşır; Secure, HttpOnly, SameSite=Lax ve __Host kapsamındadır.
- Metinler Rust tarafında çevrilir; JavaScript çeviri için gerekli değildir. Dil seçicideki uygula düğmesi JavaScript kapalıyken de çalışır.
- `crates/manager/locales/{tr,en,ar}.json` sözlükleri arayüz, durum, hata, denetim eylemi ve yardım metinlerini içerir. Sunucu/anahtar/grup adları, e-postalar, sırlar ve JSON ayar anahtarları çevrilmez. Oluşturulan CSV'nin başlıkları seçilen dilde sunulur.
- Dil bağlamı isteğe özeldir; eşzamanlı istekler arasında paylaşılmaz. Tercih değişikliğindeki dönüş adresi sınırlı iç sayfalarla doğrulanır.
- Arapçada HTML `lang=ar`, `dir=rtl`; menü sağdadır. E-posta, sayısal birimler ve teknik girdiler ayrı yazı yönünde tutulur. Tüm dillerde gösterilen saatler İstanbul (UTC+03) saatidir.

## Yardım ve yanlışlıkla yapılan işlemler

Soru işaretleri sunucu yönetimi, VPN bağlantı adresi, gruplar, bağlantı kimliği, anahtar oluşturma, cihaz sınırı, VPN oturumları, panel kullanıcıları, denetim ve hesap güvenliği açıklamalarını açar. Yerel HTML dialog öğesi kullanılır; Escape kapatır, odak açan düğmeye döner. Metinler `textContent` ile doldurulur. Bu küçük yardım pencereleri JavaScript kullanır.

Bağlantı kimliği yenileme bağlantısı artık GET ile açıklamalı onay ekranı açar; tek başına kimliği değiştirmez. Sunucu adı, eski kimliğin hemen geçersizleşeceği ve yeni ayarın sunucuya uygulanması gerektiği gösterilir. Onay kutusu ve ayrı gönderim gerekir. Bu akış JavaScript kapalıyken de çalışır.

POST işlemi rol, kiracı, kaynak (Origin), CSRF, açık onay ve güncel kimliğe bağlı onay kanıtını doğrular. Kanıt sunucu kimliği özeti ve oturumun CSRF değerinden türetilir. Satır kilidi altında doğrulanıp kimlik değiştirilir. Eski formun tekrar gönderilmesi 409 ile reddedilir; ikinci kez kimlik üretilmez. Çıplak POST isteği 400 alır. GET onayı başka kiracı için 404, salt okunur rol için 403 verir.

Anahtar iptali, toplu anahtar değişiklikleri ve VPN oturumu sonlandırma da tarayıcıda açıklamalı onay penceresi açar. Bu işlemler için sunucudaki mevcut yetki/CSRF kontrolleri sürer. Bağlantı kimliğinin ek sunucu onay akışından farklı olarak bu küçük onay pencereleri JavaScript kullanır.

## Doğrulama

- `cargo clippy --workspace --all-targets --locked -- -D warnings` başarılı.
- Çalışma alanındaki 10 test başarılı; eşzamanlı dil ayrımı, sözlük kapsamı, açık yönlendirme engeli ve HTML kaçırma dahil.
- `tests/test_panel.py`, gerçek HTTPS/PostgreSQL/.NET ile ayrı sınama kiracılarında başarılı. Onaysız/geçersiz/tekrarlanmış onay reddi, geçerli yenileme, eski/yeni agent kimlikleri, kiracı ve rol ayrımı, üç dil, güvenli çerezler, SSE ve mevcut panel işlemleri sınandı.
- Uygulama içi tarayıcıda yardım/Escape/odak dönüşü, kimlik onayını açıp vazgeçme, dil geçişleri, Arapça yön ve dar ekran taşması görsel olarak kontrol edildi. Sonuçlar `docs/dogrulama.md` dosyasında kaydedilir.

Kullanıcının mevcut “Sınama B” kimliği bu geliştirmede tekrar yenilenmedi. Daha önce yenilenen kimliği sunucuya uygulamak ayrı bir işletim adımıdır.

## Onaylanan açık/koyu tasarım — 16 Ekim 2026

Kullanıcının FortiGate, Proxmox ve AtlasHub referanslarıyla onayladığı yeni kompozisyon uygulandı. Büyük karşılama alanı yerini kısa çalışma alanı başlığına bıraktı. Dört gerçek sayaç üstte; altında hizalı sunucu durum tablosu ve hızlı erişim paneli var. Panel verileri ve yönetim izinleri değişmedi.

Açık temada kırık beyaz zemin, beyaz paneller ve sıcak gri sınırlar; koyu temada nötr antrasit yüzeyler kullanılır. Mor ana eylem rengidir; seçili menü ve sınırlı vurgularda kehribar bulunur. Yeşil, sarı ve kırmızı durum anlamlarını korur. Gradyen, parlak düğme ve kabartma etkileri kaldırılmıştır. Paletler CSS değişkenleriyle tanımlıdır.

Tema, `__Host-vhm_theme` tercih çereziyle bir yıl saklanır (Secure, HttpOnly, SameSite=Lax, kök yol). Varsayılan açıktır. Rust ilk HTML'yi tercihle oluşturur; sayfa başında farklı tema parlaması oluşmaz. Yerel form kontrollerinin `color-scheme` değeri de seçime uyar.

JavaScript çalışırken tema seçimi aynı kaynak üzerindeki `/theme` tercih uç noktasından 204 yanıtıyla kaydedilir; sayfa yenilenmez ve form verileri korunur. Hata halinde önceki seçim korunup yerelleştirilmiş hata gösterilir. JavaScript kapalıyken standart GET formu ve uygula düğmesi çalışır. Yardım ve kimlik yenileme onay akışları korunmuştur.

`tests/test_preferences.py` kimliksiz, gerçek HTTPS üzerinde tema/dil tercihlerini sınar; herhangi bir sunucu veya erişim anahtarını değiştirmez.
