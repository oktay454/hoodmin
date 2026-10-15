# İlk aşama doğrulaması — 15 Ekim 2026

## Ortam

- Ana depo: kullanıcı makinesindeki YapayZeka/VpnHood, main dalı. Uzak Git adresi kullanıcı tarafından eklenecek.
- yapayzeka: Debian 13.7 amd64, 10.0.3.236.
- vpnhood: Debian 13.7 amd64, 10.0.3.220.
- Mevcut VpnHood kaynağı: 9.0.856, kardeş VpnHood-Current dizini; ana makinedeki kaynak değiştirilmedi.

## Kurulanlar

- yapayzeka: Debian rustup 1.27.1 ve rsync 3.5.0; Microsoft deposundan .NET SDK 10.0.402 ve ilişkili 10.0.13 çalışma zamanı/hedefleme paketleri.
- Resmî Rust dağıtımından Rust/Cargo 1.99.1 kararlı, rustfmt, Clippy ve wasm32-unknown-unknown standart kitaplığı. Araç zinciri /opt/vpnhood-toolchain altında; proje sürümü sabitler. Debian rustc/cargo paketlerinin yerini rustup yönlendirmeleri aldı.
- crates.io üzerinden Cargo Audit 0.22.2 ve Cargo.lock ile sabitlenmiş proje bağımlılıkları.
- vpnhood: Microsoft deposundan .NET runtime 10.0.13, Debian rsync ve bağımlılıkları.
- PostgreSQL 17, Nginx, derleyici/paketleme araçları önceden mevcuttu.

APT her iki kapsayıcıda Microsoft deposunun .list ve .sources biçiminde iki kez tanımlandığına dair uyarı verdi. Kullanıcının kaynak tanımları değiştirilmedi; bu durum kurulumu engellemedi.

## Çalışan sınama servisleri

- yapayzeka: vpnhood-manager.service, vhmgr kullanıcısı, 127.0.0.1:3080.
- Nginx test TLS uç noktası: https://10.0.3.236:3443; özel CA, 30 günlük sertifika. Yalnızca LXC özel adresinde dinler. Vekil kullanım bildirimlerini kendiliğinden tekrar göndermez.
- vpnhood: vpnhood-test.service, özgün VpnHoodServer.dll, 0.0.0.0:2443.
- Sunucu yapılandırmayı aldı, Server is ready durumuna geçti; veri tabanında machineName=vpnhood ve configError boş olarak doğrulandı.
- CA güveni yalnızca vpnhood kapsayıcısına eklendi. TLS doğrulaması kapatılmadı; HTTP dinleyici yalnızca yöneticinin loopback adresindedir.

## Geçen sınamalar

- cargo test --workspace --locked: 3 Rust testi geçti.
- cargo clippy --workspace --all-targets --locked -- -D warnings: geçti.
- cargo audit --deny warnings: geçti; 221 kilitli bağımlılıkta RustSec eşleşmesi/uyarısı yok. Bu sonuç genel bir güvenlik garantisi değildir.
- İlk denetimde kullanılmayan SQLx MySQL sürücüsünün rsa bağımlılığı RUSTSEC-2023-0071 bildirimi üretti. MySQL/SQLite/makro bağımlılıkları tamamen çıkarıldı; yalnızca sqlx-core ve sqlx-postgres kullanılıyor. Uyarı bastırılmadı.
- Gerçek .NET HttpAccessManager ile yapılandırma, sunucu kimliği, sahte kimliğin reddi, istemci kanıtı, anahtar/kiracı sınırı, oturum açma, okuma, listeleme, kapatma ve tekrar açma.
- Tekli ve toplu trafik farkı, durum bildirimi, kota aşımı, negatif trafik ve u64 kimlik sınırı.
- 16 farklı istemcinin eşzamanlı oturum açma isteğinde yalnızca bir isteğin kabul edilmesi, diğer 15 isteğin cihaz kotası nedeniyle reddi.
- PostgreSQL RLS: bağlamsız sorgunun sıfır satır döndürmesi, başka kiracının görünmemesi ve kiracılar arası dış anahtar ilişkisinin reddi.
- Gerçek VpnHoodClient: Rust yöneticinin ürettiği vh:// anahtarla uzak kapsayıcıdaki gerçek sunucuya bağlantı kuruldu. Sertifika eşleşmesi VpnHood'un normal doğrulamasından geçti.

## Sınırlar

Bağlantı sınaması NullVpnAdapter kullanır; işletim sisteminin VPN yönlendirmesini veya tünel üzerinden kullanıcı trafiğini taşıma başarımını sınamaz. Panel, kullanıcı rolleri, kalıcı işler, ACME, grup içi yük dağıtımı ve Debian paketleme sonraki aşamalardır. Sınama kayıtları ayrılmış veri tabanında bırakıldı; özel veriler work altında ve Git dışında kalır.
