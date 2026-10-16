# Onaylanan mimari ve kiracı modeli

Her kullanıcı kuruluşu bir kiracıdır (tenant). Bir kiracı birden fazla yöneticiyi, sunucu grubunu (server farm), sunucuyu ve erişim anahtarını barındırabilir. Bir sunucu tek kiracıya aittir; ortak sunucu havuzu yoktur. Anahtarlar şu anda tek sunucuya bağlıdır. Grup yönlendirmesi tamamlandığında yalnız aynı kiracının sunucuları arasında kullanılacaktır.

## İletişim yönü

VpnHood sunucusu yöneticinin `/api/agent/` API'sine HTTPS isteği yapar. `configure` yapılandırmayı döndürür; `status` yanıtı yapılandırma sürümünü ve oturum kararlarını iletir. Yönetici VPN trafiğini taşımaz. İstemci doğrudan VPN sunucusuna bağlanır. SSH yalnız kurulum ve bakım iş akışına aittir.

## Bileşenler

Rust/Tokio/Axum, PostgreSQL/SQLx ve Leptos SSR tek systemd servisi içinde modüler uygulama olarak çalışır. Panel standart HTML formlarıyla kullanılır; küçük JavaScript dosyası toplu seçim, kopyalama ve SSE sayaçlarını sağlar. WebAssembly araç zinciri hazırdır ancak bu aşamada tarayıcıda WASM veya hidrasyon (hydration) kullanılmaz. Böylece sırlar ve yetki kararları sunucuda kalır.

Nginx TLS sonlandırır; Rust yalnız loopback üzerinde dinler. PostgreSQL UNIX soketi, ayrı sistem kullanıcısı ve peer authentication kullanılır. Kalıcı iş kuyruğu, SSH iş yürütücüsü ve Debian kaynak/ikili paketleri sonraki aşamadadır.

## Kimlik ve yetki

Küresel hesaplar e-posta/Argon2id parola özetini, oturumlar ise rastgele 256 bit oturum sırrının SHA-256 özetini tutar. Giriş denemeleri e-posta ve Nginx'in ilettiği gerçek istemci IP adresi üzerinden sınırlanır. Panel çerezleri Secure, HttpOnly, SameSite=Strict ve __Host kapsamındadır. Oturum 8 saat geçerlidir; parola değişimi tüm oturumları, kullanıcı kapatma mevcut oturumları geçersizleştirir. Üyelik rolü her istekte yeniden doğrulanır.

Yönetici kullanıcıları ve çalışma alanını yönetir; işletmen sunucu/anahtar/oturum işlemlerini yapar; gözlemci yalnız okur. Kimlik doğrulaması, rol, CSRF ve Origin kontrolleri tüm değişiklik uç noktalarında sunucuda uygulanır. Son yönetici kontrolü kiracı satır kilidiyle eşzamanlı değişikliklerden korunur. Çoklu kiracı üyeliği şemada mümkün olsa da panel şu anda hesabın varsayılan kiracısında çalışır.

## Kiracı ayrımı

Kiracı kimliği istek gövdesinden alınmaz: kullanıcı oturumundan veya doğrulanan sunucu kimliğinden türetilir. Sunucu kimliği tenant UUID, sunucu UUID ve 256 bit rastgele sır içerir; yalnız bütün kimliğin SHA-256 özeti veri tabanındadır. UUID bilmek erişim sağlamaz. Her işlemde kiracı bağlamı işlem kapsamıyla ayarlanır; bağlantı havuzunda kiracı durumu taşınmaz.

Bütün kiracılı kaynak tablolarında RLS ve FORCE RLS uygulanır. Giriş öncesi kimlik tabloları küreseldir; kaynak sorguları kiracı bağlamında ve RLS altında çalışır. Uygulama ayrı, RLS atlayamayan rol kullanır. Dış anahtarlar kiracı kimliğini içerir. Yapılandırma ve sırlar kaynak kimliğine bağlanan AES-256-GCM ek doğrulanmış verisiyle (additional authenticated data, AAD) korunur. Denetim kayıtlarının UPDATE/DELETE yetkisi uygulama rolünden alınmıştır.

## Oturum kararları

Cihaz sayımı ve oturum açma aynı anahtar satırı kilidi altında yapılır. Panelde oturum kapatma veya anahtar politikası değişikliği kalıcı `pending_notify` işareti oluşturur. Sunucuya durum yanıtında karar iletilir; kapanış onayı gelene kadar tekrar gönderilir. Kota hesaplaması VpnHood kullanım raporlarının sıklığına bağlıdır. Muhasebe sözleşmesi tekil rapor kimliği sağlamadığından kesintilerde tam bir kez sayım garanti edilmez.

## Aşamalar

1. Tamamlandı: gerçek .NET protokol uyumluluğu, kiracı ayrımı, oturum/kota ve uzak istemci bağlantısı.
2. Tamamlandı: kimlik doğrulaması, roller, Leptos paneli, grup kaydı, toplu anahtar işlemleri, raporlar ve denetim kaydı.
3. Bekliyor: SSH kurulum/bakım, kalıcı işler, grup yönlendirmesi/yük dağıtımı, ACME, Debian paketleme ve üretim işletim doğrulaması.

## Sınama yerleşimi

`yapayzeka` (10.0.3.236): araç zinciri, PostgreSQL, Rust yönetici ve Nginx. Panel `https://vhm-test.oktayaktogan.com.tr`; VPS Nginx (`43.229.92.120`), `https://46.196.34.83:40443` üzerinden kapsayıcı Nginx'ine bağlanır. Rust `127.0.0.1:3080`. Eski `https://yapayzeka:3443/api/agent/` sınama sunucuları için korunur. Geçiş ayrıntıları `docs/vps-ters-vekil.md` içindedir. `vpnhood` (10.0.3.220): mevcut gerçek VpnHood sunucusu port 2443, panelden oluşturulan ayrı sınama sunucusu port 2444. Ağ lxcbr0. Mevcut Nginx port 80 ile çakışma yoktur. Ek kapsayıcı henüz gerekmemiştir.
