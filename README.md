# Hoodmin

[Türkçe](README.md) · [English](README.en.md) · [العربية](README.ar.md)

Hoodmin, VpnHood ortamları için Rust ile yazılmış bağımsız bir yönetim panelidir. Kuruluşlar kendi VPN sunucularını, erişim anahtarlarını, oturumlarını ve sertifikalarını ayrı çalışma alanlarında yönetir. İstemciler VPN trafiğini doğrudan VpnHood sunucusuna gönderir.

- Linux üzerinde VpnHood sunucusu kurulumu, SSH, sunucu profilleri ve grup yönlendirmesi.
- Toplu erişim anahtarı, erişim profili, hız, cihaz, trafik ve süre sınırları.
- PFX sertifikaları, Let's Encrypt HTTP-01 AutoSign ve şifreli adres yayımı.
- Kullanım raporları, denetim kaydı, ekip rolleri, botlar ve JSON API.
- Türkçe, İngilizce ve Arapça arayüz, açık ve koyu tema.
- Yerel veya uzak PostgreSQL, otomatik Debian yükseltmesi, şifreli yedekler, anahtar döndürme ve bildirimler.

Debian 13 amd64 ve VpnHood 9.0.856 doğrulama tabanıdır. Panel, resmî VpnHood sunucu ve istemcileriyle çalışır.

[Debian kurulumu](docs/uretim-kurulumu.md) · [Kullanıcı elkitabı](docs/elkitabi.md) · [İşletim ve teknik belgeler](docs/index.md)

## Lisans

Hoodmin özgür yazılımdır. GNU Genel Kamu Lisansı sürüm 3 veya seçiminize göre daha sonraki bir sürüm (`GPL-3.0-or-later`) ile lisanslanır. [Lisans metni](LICENSE).
