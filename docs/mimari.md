# Onaylanan mimari ve kiracı modeli

Her kullanıcı kuruluşu bir kiracıdır (tenant). Bir kiracı birden fazla yöneticiyi, sunucu grubunu (server farm), sunucuyu ve erişim anahtarını barındırabilir. Bir sunucu tek kiracıya aittir; ortak sunucu havuzu yoktur. İlk aşamada anahtarlar tek sunucuya bağlıdır. Grup desteği eklendiğinde aynı kiracının sunucuları arasında yönlendirme yapılacaktır.

## İletişim yönü

VpnHood sunucusu yöneticinin `/api/agent/` API'sine HTTPS isteği yapar. `configure` yapılandırmayı döndürür; `status` durum bildirimi yanıtında yapılandırma sürümünü ve oturum kararlarını döndürür. Yönetici VPN trafiğini taşımaz. İstemci doğrudan VPN sunucusuna bağlanır. SSH yalnızca kurulum ve bakım iş akışına aittir.

## Bileşenler

Rust/Tokio/Axum sunucu uygulaması, Leptos SSR + WebAssembly arayüzü, PostgreSQL/SQLx veri katmanı, PostgreSQL kalıcı iş kuyruğu ve Rust iş yürütücüsü. Panel canlı güncellemeleri SSE ile alacaktır. Başlangıçta modüler tek uygulama (modular monolith), tek systemd servisi. Debian standartlarına uygun kaynak ve ikili paketleme sonraki aşamada eklenecektir.

## Ayrım

Kiracı kimliği istek gövdesinden alınmaz: kullanıcı oturumundan veya doğrulanan sunucu kimliğinden türetilir. Sunucu kimliği tenant UUID, sunucu UUID ve 256 bit rastgele sır içerir; yalnızca bütün kimliğin SHA-256 özeti veri tabanındadır. UUID'lerin bilinmesi erişim sağlamaz. Her işlemde tenant bağlamı işlem kapsamıyla ayarlanır; bağlantı havuzunda kiracı durumu taşınmaz.

Bütün kiracılı tablolarda RLS ve FORCE RLS uygulanır. Uygulama ayrı, RLS atlayamayan rol kullanır. Dış anahtarlar kiracı kimliğini de içerir; bir kiracının anahtarı başka kiracının sunucusuna bağlanamaz. Platform yöneticisinin gelecekteki işlemleri ayrı ve denetlenebilir yetki yolunda olacaktır.

## Aşamalar

1. Gerçek .NET tipleri ve VpnHood sunucusuyla protokol uyumluluğu, kiracı ayrımı, oturum ve kota testleri.
2. Kullanıcı kimlik doğrulaması, roller, Leptos paneli, sunucu grupları, toplu anahtar işlemleri ve raporlar.
3. SSH kurulum/bakım, kalıcı iş kuyruğu, grup içi yük dağıtımı, sertifika yenileme, Debian paketleme ve işletim doğrulaması.

## Sınama yerleşimi

`yapayzeka` (10.0.3.236): derleme araçları, PostgreSQL ve yönetici. `vpnhood` (10.0.3.220): mevcut VpnHood sunucusu. Ağ: lxcbr0. Çoklu VPN sunucusu denemeleri için sonraki aşamada üçüncü kapsayıcı istenebilir.
