# Arayüz ve diller

Günlük kullanım akışı [Hoodmin elkitabında](elkitabi.md) anlatılır. Bu belge görünüm, dil ve etkileşim davranışına başvurudur.

## Görünüm

Açık temada kırık beyaz/sıcak gri, koyu temada nötr antrasit yüzeyler kullanılır. Renkler düzdür; gradyen yoktur. Mor ve ölçülü kehribar vurgu, belirgin kart/tablo sınırları, simgeli gezinme ve görünür **Hesabım** bağlantısı vardır. Üst çubuktaki tek güneş/ay düğmesi temayı değiştirir; tercih tarayıcıda saklanır.

Arayüz VPN yönetimini anlatır. Sunucu menüsündeki değerler VPN sunucusuna aittir; panel makinesinin genel sistem yönetimi yapılmaz. VPN hesap/anahtarları ile panel hesapları ayrıdır.

## Dil ve yerleşim

Türkçe varsayılandır; Türkçe, İngilizce ve Arapça giriş/panel ekranlarından seçilebilir. Tercih bir yıl süreli, yalnız dil kodu taşıyan Secure/HttpOnly/SameSite=Lax çereziyle saklanır. Metinler sunucuda çevrilir; JavaScript olmadan dil formu kullanılabilir.

Sözlükler `crates/manager/locales/{tr,en,ar}.json` dosyalarıdır. Kaynak adları, e-postalar, sırlar ve JSON ayar anahtarları çevrilmez. CSV başlıkları seçili dili kullanır. Dil bağlamı isteğe özeldir; Arapçada `lang=ar`, `dir=rtl` olur. E-posta ve teknik girdiler kendi yönünde gösterilir.

Genel kullanım saatleri İstanbul (UTC+03) olarak gösterilir. AutoSign ve harici yayım iş zamanları açıkça **UTC** etiketi taşır. Girdi zamanını saat dilimi içeren ISO 8601 biçiminde yazın.

## Yardım ve onay

Soru işareti düğmeleri alanın ne olduğunu ve nasıl kullanılacağını küçük yardım penceresinde gösterir. Escape kapatır, odak açan düğmeye döner. Pencere/kopyalama/toplu seçim gibi etkileşimler JavaScript kullanır.

Bağlantı kimliği yenileme önce açıklamalı onay sayfası açar. Ad, etki ve onay kutusu gösterilir; GET kimliği değiştirmez. Gönderimde güncel kimliğe bağlı onay kanıtı denetlenir; eski form 409 alır. Bu güvence JavaScript'ten bağımsızdır.

Anahtar iptali, toplu değişiklik ve oturum sonlandırmada açıklamalı tarayıcı onayı vardır. Sunucudaki rol/kiracı/CSRF/Origin denetimi her durumda uygulanır. Yapılandırma, PFX, AutoSign ve harici yayım gibi işlemler ayrıca kendi açık etki onaylarını gerektirir.
