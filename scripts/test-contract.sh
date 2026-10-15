#!/bin/bash
set -euo pipefail

PROJE_DIZINI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SINAMA_DIZINI="$(mktemp -d "${PROJE_DIZINI}/work/contract.XXXXXX")"
KIRACI_A="$(cat /proc/sys/kernel/random/uuid)"
KIRACI_B="$(cat /proc/sys/kernel/random/uuid)"
export DATABASE_URL="postgresql:///vpnhood_manager?host=/var/run/postgresql&user=vhmgr"
export VHM_MASTER_KEY_FILE="/var/lib/vpnhood-manager/master.key"
export DOTNET_CLI_TELEMETRY_OPTOUT=1
export DOTNET_NOLOGO=1

function Yonet()
{
	runuser -u vhmgr --preserve-environment -- /usr/lib/vpnhood-manager/vpnhood-manager "$@"
}

function Sina()
{
	dotnet build "${PROJE_DIZINI}/tests/ContractProbe" -c Release --nologo
	dotnet run --project "${PROJE_DIZINI}/tests/ContractProbe" -c Release --no-build -- prepare "${SINAMA_DIZINI}"
	chown -R vhmgr:vhmgr "${SINAMA_DIZINI}"
	Yonet register-server --tenant "${KIRACI_A}" --name 'Sınama A' --configuration "${SINAMA_DIZINI}/server-config.json" --credential-out "${SINAMA_DIZINI}/server-a.credential"
	Yonet register-server --tenant "${KIRACI_B}" --name 'Sınama B' --configuration "${SINAMA_DIZINI}/server-config.json" --credential-out "${SINAMA_DIZINI}/server-b.credential"
	SUNUCU_A="$(cut -d . -f 2 "${SINAMA_DIZINI}/server-a.credential")"
	SUNUCU_B="$(cut -d . -f 2 "${SINAMA_DIZINI}/server-b.credential")"
	Yonet create-token --tenant "${KIRACI_A}" --server "${SUNUCU_A}" --name 'Sınama A' --max-traffic 4096 --key-out "${SINAMA_DIZINI}/token-a.vh"
	Yonet create-token --tenant "${KIRACI_B}" --server "${SUNUCU_B}" --name 'Sınama B' --key-out "${SINAMA_DIZINI}/token-b.vh"
	Yonet create-token --tenant "${KIRACI_B}" --server "${SUNUCU_B}" --name 'Süresi dolmuş' --expires-at '2000-01-01T00:00:00Z' --key-out "${SINAMA_DIZINI}/token-expired.vh"
	IPTAL_EDILECEK_ID="$(Yonet create-token --tenant "${KIRACI_B}" --server "${SUNUCU_B}" --name 'İptal edilecek' --key-out "${SINAMA_DIZINI}/token-revoked.vh" | sed -n 's/^Erişim anahtarı oluşturuldu: \([0-9a-f-]*\)\..*/\1/p')"
	Yonet revoke-token --tenant "${KIRACI_B}" --token "${IPTAL_EDILECEK_ID}"
	dotnet run --project "${PROJE_DIZINI}/tests/ContractProbe" -c Release --no-build -- probe "${SINAMA_DIZINI}"
	# RLS: kimliksiz sorgu satır döndürmemeli; başka kiracıya ilişki kurulması reddedilmeli.
	runuser -u vhmgr -- psql -v ON_ERROR_STOP=1 -d vpnhood_manager <<SQL
DO \$\$
BEGIN
	IF (SELECT count(*) FROM servers) <> 0 THEN
		RAISE EXCEPTION 'Kimliksiz sorgu RLS sınırını aştı';
	END IF;
END \$\$;
BEGIN;
SELECT set_config('app.tenant_id','${KIRACI_A}',true);
DO \$\$
BEGIN
	IF EXISTS (SELECT FROM servers WHERE tenant_id='${KIRACI_B}') THEN
		RAISE EXCEPTION 'Başka kiracının sunucusu görünür';
	END IF;
	BEGIN
		INSERT INTO access_tokens(tenant_id,id,server_id,name,secret)
		VALUES('${KIRACI_A}',gen_random_uuid(),'${SUNUCU_B}','Geçersiz ilişki','\\x00');
		RAISE EXCEPTION 'Kiracılar arası sunucu ilişkisi kabul edildi';
	EXCEPTION WHEN foreign_key_violation THEN
		NULL;
	END;
END \$\$;
ROLLBACK;
SQL
	printf '%s\n' 'PASS: kimliksiz ve kiracılar arası SQL erişimi RLS/dış anahtar ile engellendi.'
	printf '%s\n' "SINAMA_DIZINI=${SINAMA_DIZINI}"
}

Sina
