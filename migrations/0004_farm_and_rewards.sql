BEGIN;
ALTER TABLE servers ADD COLUMN management jsonb NOT NULL DEFAULT '{}';
ALTER TABLE farms ADD COLUMN management jsonb NOT NULL DEFAULT '{}';
ALTER TABLE access_tokens ADD COLUMN farm_id uuid;
ALTER TABLE access_tokens ADD CONSTRAINT token_farm_owner FOREIGN KEY(tenant_id,farm_id) REFERENCES farms(tenant_id,id);
ALTER TABLE sessions DROP CONSTRAINT sessions_tenant_id_server_id_token_id_fkey;
ALTER TABLE sessions ADD CONSTRAINT session_token_owner FOREIGN KEY(tenant_id,token_id) REFERENCES access_tokens(tenant_id,id);
ALTER TABLE sessions ADD CONSTRAINT session_server_owner FOREIGN KEY(tenant_id,server_id) REFERENCES servers(tenant_id,id);
ALTER TABLE sessions ADD COLUMN ad_verified boolean NOT NULL DEFAULT false;
CREATE TABLE access_codes (
	tenant_id uuid NOT NULL,
	code_hash bytea NOT NULL CHECK(octet_length(code_hash)=32),
	token_id uuid NOT NULL,
	created_at timestamptz NOT NULL DEFAULT now(),
	PRIMARY KEY(tenant_id,code_hash),
	FOREIGN KEY(tenant_id,token_id) REFERENCES access_tokens(tenant_id,id)
);
ALTER TABLE access_codes ENABLE ROW LEVEL SECURITY;
ALTER TABLE access_codes FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON access_codes USING(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid);
CREATE TABLE ad_hooks (
	tenant_id uuid PRIMARY KEY REFERENCES tenants(id),
	credential_hash bytea NOT NULL CHECK(octet_length(credential_hash)=32)
);
ALTER TABLE ad_hooks ENABLE ROW LEVEL SECURITY;
ALTER TABLE ad_hooks FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON ad_hooks USING(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid);
CREATE TABLE ad_rewards (
	tenant_id uuid NOT NULL,
	session_id bigint NOT NULL,
	ad_hash bytea NOT NULL CHECK(octet_length(ad_hash)=32),
	expires_at timestamptz NOT NULL DEFAULT now()+interval '15 minutes',
	consumed boolean NOT NULL DEFAULT false,
	PRIMARY KEY(tenant_id,ad_hash),
	FOREIGN KEY(tenant_id,session_id) REFERENCES sessions(tenant_id,id)
);
ALTER TABLE ad_rewards ENABLE ROW LEVEL SECURITY;
ALTER TABLE ad_rewards FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON ad_rewards USING(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid);
-- Existing non-cyclic counters must retain the same meaning after migration.
UPDATE access_tokens SET cycle_sent=sent,cycle_received=received WHERE cycle_started_at IS NULL AND cycle_sent=0 AND cycle_received=0;
COMMIT;
