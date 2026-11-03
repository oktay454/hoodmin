# Arayüz ve diller

Günlük kullanım akışı [Hoodmin elkitabında](elkitabi.md) anlatılır. Bu belge görünüm, dil ve etkileşim davranışına başvurudur.

## Görünüm

Açık temada kırık beyaz/sıcak gri, koyu temada nötr antrasit yüzeyler kullanılır. Renkler düzdür; gradyen yoktur. Mor ve ölçülü kehribar vurgu, belirgin kart/tablo sınırları, ayrılmış gezinme ve görünür **Hesabım** bağlantısı vardır. Üst çubuktaki tek güneş/ay düğmesi temayı değiştirir; tercih tarayıcıda saklanır.

Arayüz VPN yönetimini anlatır. Sunucu menüsündeki değerler VPN sunucusuna aittir; panel makinesinin genel sistem yönetimi yapılmaz. VPN hesap/anahtarları ile panel hesapları ayrıdır.

## Dil ve yerleşim

Türkçe varsayılandır; Türkçe, İngilizce ve Arapça giriş/panel ekranlarından seçilebilir. Dil ve tema yalnız tercih olarak tarayıcının yerel deposunda (localStorage) saklanır. Oturum sırrı burada tutulmaz. Statik sözlük yüklenir ve metinler tarayıcıda çevrilir; arayüz JavaScript gerektirir.

Sözlükler `crates/manager/web/assets/locales/{tr,en,ar}.json` dosyalarıdır. Kaynak adları, e-postalar, sırlar ve JSON ayar anahtarları çevrilmez. Teknik API/CSV yanıtlarının dili arayüz tercihinden bağımsız olabilir; Arapçada `lang=ar`, `dir=rtl` olur. E-posta ve teknik girdiler kendi yönünde gösterilir.

Genel kullanım saatleri İstanbul (UTC+03) olarak gösterilir. AutoSign ve harici yayım iş zamanları açıkça **UTC** etiketi taşır. Tarih alanlarında yerel saat seçimi API için saat dilimli ISO 8601 değerine dönüştürülür.

## Yardım ve onay

Soru işareti düğmeleri alanın ne olduğunu ve nasıl kullanılacağını küçük yardım penceresinde gösterir. Escape kapatır, odak açan düğmeye döner. Pencere/kopyalama/toplu seçim gibi etkileşimler JavaScript kullanır.

Bağlantı kimliği yenileme önce açıklamalı onay sayfası açar. Etki ve ayrı Onayla/Vazgeç düğmeleri gösterilir; GET kimliği değiştirmez. Gönderimde güncel kimliğe bağlı onay kanıtı denetlenir; eski form 409 alır. Güncel kanıt ve açık onay API tarafında da zorunludur.

Anahtar iptali, toplu değişiklik ve oturum sonlandırmada açıklamalı tarayıcı onayı vardır. Sunucudaki rol/kiracı/CSRF/Origin denetimi her durumda uygulanır. Yapılandırma, PFX, AutoSign ve harici yayım gibi işlemler ayrıca kendi açık etki onaylarını gerektirir.

## Veri ve kullanılabilirlik

Bütün sayfaların HTML iskeleti, JavaScript, CSS ve dil sözlükleri sabittir. Tarayıcı `/api/manage` üzerinden JSON alıp ekranı oluşturur; HTML parçası indirmez. Veritabanı kapalıyken de arayüz açılır ve basit hata gösterip tekrar dener. Yanıt sırları önbelleğe alınmaz; oturum çerezi HttpOnly/Secure olarak kalır. Gezinme ve temalar tam sayfa yenilemesi gerektirmez.

Statik dosyaların tek kurulu konumu `/usr/share/hoodmin/web/` ağacıdır. Hoodmin veya Nginx aynı dosyaları sunar; ayrı ön uç makinesine yalnız bu dizin kopyalanır. Arayüz `/` kökünü kullanır; `/panel` ve `/manage` önekleri yoktur. [Sunum ve kopyalama adımları](web-sunumu.md).
