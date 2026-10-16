BEGIN;
CREATE TABLE server_profiles (
	tenant_id uuid NOT NULL REFERENCES tenants(id),
	id uuid NOT NULL,
	name text NOT NULL CHECK(length(name) BETWEEN 1 AND 120),
	settings jsonb NOT NULL,
	management jsonb NOT NULL,
	version uuid NOT NULL,
	PRIMARY KEY(tenant_id,id),
	UNIQUE(tenant_id,name)
);
ALTER TABLE server_profiles ENABLE ROW LEVEL SECURITY;
ALTER TABLE server_profiles FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON server_profiles USING(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid);
ALTER TABLE servers ADD COLUMN heartbeat_seconds integer NOT NULL DEFAULT 90 CHECK(heartbeat_seconds BETWEEN 3 AND 259200);
COMMIT;
