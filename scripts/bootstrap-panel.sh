#!/bin/bash
set -euo pipefail

PROJE_DIZINI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KIRACI="${1:?Kiracı UUID kimliği gerekli.}"
PAROLA_DOSYASI="${PROJE_DIZINI}/work/panel-admin.password"
BILGI_DOSYASI="${PROJE_DIZINI}/work/panel-admin.txt"
export DATABASE_URL="postgresql:///vpnhood_manager?host=/var/run/postgresql&user=hoodmin"
export VHM_MASTER_KEY_FILE="/var/lib/vpnhood-manager/master.key"

function Hazirla()
{
	if [ -e "${BILGI_DOSYASI}" ] || [ -e "${PAROLA_DOSYASI}" ]
	then
		printf '%s\n' 'Başlangıç bilgisi zaten var; hesap/parola değiştirilmedi.' >&2
		exit 1
	fi
	umask 077
	openssl rand -base64 24 >"${PAROLA_DOSYASI}"
	chown hoodmin:hoodmin "${PAROLA_DOSYASI}"
	runuser -u hoodmin --preserve-environment -- /usr/lib/vpnhood-manager/vpnhood-manager create-account --tenant "${KIRACI}" --tenant-name 'Hoodmin çalışma alanı' --email 'admin@vpnhood.test' --password-file "${PAROLA_DOSYASI}" --role admin
	{
		printf '%s\n' 'Adres: https://yapayzeka:3443' 'E-posta: admin@vpnhood.test'
		printf 'Parola: '
		cat "${PAROLA_DOSYASI}"
	} >"${BILGI_DOSYASI}"
	chmod 0600 "${BILGI_DOSYASI}"
	printf '%s\n' "Giriş bilgileri özel dosyada: ${BILGI_DOSYASI}"
}

Hazirla
