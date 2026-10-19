#!/bin/bash
set -euo pipefail

PROJE_DIZINI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERI_DIZINI="/var/lib/vpnhood-manager"

function Hazirla()
{
	if [ "$(id -u)" -ne 0 ]
	then
		printf '%s\n' 'Bu betik yalnızca sınama kapsayıcısında root ile çalıştırılır.' >&2
		exit 1
	fi
	if ! getent passwd hoodmin >/dev/null
	then
		useradd --system --home-dir "${VERI_DIZINI}" --shell /usr/sbin/nologin hoodmin
	fi
	install -d -o hoodmin -g hoodmin -m 0700 "${VERI_DIZINI}"
	if [ ! -f "${VERI_DIZINI}/master.key" ]
	then
		umask 077
		head -c 32 /dev/urandom >"${VERI_DIZINI}/master.key"
		chown hoodmin:hoodmin "${VERI_DIZINI}/master.key"
	fi
	runuser -u postgres -- psql -v ON_ERROR_STOP=1 <<'SQL'
DO $$
BEGIN
	IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname='hoodmin') THEN
		CREATE ROLE hoodmin LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOBYPASSRLS;
	END IF;
END $$;
SQL
	if [ "$(runuser -u postgres -- psql -Atqc "SELECT count(*) FROM pg_database WHERE datname='vpnhood_manager'")" -eq 0 ]
	then
		runuser -u postgres -- createdb vpnhood_manager
	fi
	if [ "$(runuser -u postgres -- psql -d vpnhood_manager -Atqc "SELECT to_regclass('public.tenants') IS NULL")" = t ]
	then
		runuser -u postgres -- psql -v ON_ERROR_STOP=1 -d vpnhood_manager -f "${PROJE_DIZINI}/migrations/0001_initial.sql"
	fi
	if [ "$(runuser -u postgres -- psql -d vpnhood_manager -Atqc "SELECT to_regclass('public.accounts') IS NULL")" = t ]
	then
		runuser -u postgres -- psql -v ON_ERROR_STOP=1 -d vpnhood_manager -f "${PROJE_DIZINI}/migrations/0002_panel.sql"
	fi
	if [ "$(runuser -u postgres -- psql -d vpnhood_manager -Atqc "SELECT to_regclass('public.access_profiles') IS NULL")" = t ]
	then
		runuser -u postgres -- psql -v ON_ERROR_STOP=1 -d vpnhood_manager -f "${PROJE_DIZINI}/migrations/0003_management_contract.sql"
	fi
	if [ "$(runuser -u postgres -- psql -d vpnhood_manager -Atqc "SELECT to_regclass('public.ad_hooks') IS NULL")" = t ]
	then
		runuser -u postgres -- psql -v ON_ERROR_STOP=1 -d vpnhood_manager -f "${PROJE_DIZINI}/migrations/0004_farm_and_rewards.sql"
	fi
	if [ "$(runuser -u postgres -- psql -d vpnhood_manager -Atqc "SELECT to_regclass('public.install_jobs') IS NULL")" = t ]
	then
		runuser -u postgres -- psql -v ON_ERROR_STOP=1 -d vpnhood_manager -f "${PROJE_DIZINI}/migrations/0005_install_jobs.sql"
	fi
	if [ "$(runuser -u postgres -- psql -d vpnhood_manager -Atqc "SELECT to_regclass('public.server_profiles') IS NULL")" = t ]
	then
		runuser -u postgres -- psql -v ON_ERROR_STOP=1 -d vpnhood_manager -f "${PROJE_DIZINI}/migrations/0006_server_profiles.sql"
	fi
	if [ "$(runuser -u postgres -- psql -d vpnhood_manager -Atqc "SELECT to_regclass('public.bots') IS NULL")" = t ]
	then
		runuser -u postgres -- psql -v ON_ERROR_STOP=1 -d vpnhood_manager -f "${PROJE_DIZINI}/migrations/0007_team_bots.sql"
	fi
	if [ "$(runuser -u postgres -- psql -d vpnhood_manager -Atqc "SELECT count(*) FROM information_schema.columns WHERE table_name='farms' AND column_name='publication_origin'")" -eq 0 ]
	then
		runuser -u postgres -- psql -v ON_ERROR_STOP=1 -d vpnhood_manager -f "${PROJE_DIZINI}/migrations/0008_farm_publication.sql"
	fi
	if [ "$(runuser -u postgres -- psql -d vpnhood_manager -Atqc "SELECT to_regclass('public.external_publications') IS NULL")" = t ]
	then
		runuser -u postgres -- psql -v ON_ERROR_STOP=1 -d vpnhood_manager -f "${PROJE_DIZINI}/migrations/0009_external_publications.sql"
	fi
	if [ "$(runuser -u postgres -- psql -d vpnhood_manager -Atqc "SELECT count(*) FROM information_schema.columns WHERE table_name='farms' AND column_name='server_profile_id'")" -eq 0 ]
	then
		runuser -u postgres -- psql -v ON_ERROR_STOP=1 -d vpnhood_manager -f "${PROJE_DIZINI}/migrations/0010_farm_server_profiles.sql"
	fi
	if [ "$(runuser -u postgres -- psql -d vpnhood_manager -Atqc "SELECT to_regclass('public.acme_orders') IS NULL")" = t ]
	then
		runuser -u postgres -- psql -v ON_ERROR_STOP=1 -d vpnhood_manager -f "${PROJE_DIZINI}/migrations/0011_acme_orders.sql"
	fi
	runuser -u postgres -- psql -v ON_ERROR_STOP=1 -d vpnhood_manager <<'SQL'
GRANT CONNECT ON DATABASE vpnhood_manager TO hoodmin;
GRANT USAGE ON SCHEMA public TO hoodmin;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO hoodmin;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO hoodmin;
REVOKE UPDATE, DELETE ON audit_events FROM hoodmin;
SQL
	install -d -m 0755 /usr/lib/vpnhood-manager
	install -m 0755 "${PROJE_DIZINI}/target/release/vpnhood-manager" /usr/lib/vpnhood-manager/vpnhood-manager
	install -m 0644 "${PROJE_DIZINI}/scripts/vpnhood-manager.service" /etc/systemd/system/vpnhood-manager.service
	systemctl daemon-reload
	systemctl enable vpnhood-manager
	systemctl restart vpnhood-manager
	curl --fail --silent --retry 10 --retry-connrefused --retry-delay 1 http://127.0.0.1:3080/healthz
}

Hazirla
