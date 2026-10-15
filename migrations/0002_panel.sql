BEGIN;
CREATE TABLE accounts (
	id uuid PRIMARY KEY,
	email text NOT NULL UNIQUE CHECK (length(email) BETWEEN 3 AND 254),
	password_hash text NOT NULL,
	default_tenant_id uuid NOT NULL REFERENCES tenants(id),
	active boolean NOT NULL DEFAULT true
);
CREATE TABLE memberships (
	tenant_id uuid NOT NULL REFERENCES tenants(id),
	account_id uuid NOT NULL REFERENCES accounts(id),
	role text NOT NULL CHECK (role IN ('admin','operator','viewer')),
	PRIMARY KEY(tenant_id, account_id)
);
-- Kimlik oturumları küresel kimlik katmanına aittir; sır özetine göre tekil okunur.
CREATE TABLE web_sessions (
	token_hash bytea PRIMARY KEY CHECK(octet_length(token_hash)=32),
	tenant_id uuid NOT NULL,
	account_id uuid NOT NULL,
	csrf text NOT NULL,
	expires_at timestamptz NOT NULL,
	FOREIGN KEY(tenant_id,account_id) REFERENCES memberships(tenant_id,account_id)
);
CREATE INDEX web_sessions_expiration ON web_sessions(expires_at);
CREATE TABLE login_attempts (
	key bytea PRIMARY KEY,
	failures integer NOT NULL,
	window_start timestamptz NOT NULL
);
CREATE TABLE farms (
	tenant_id uuid NOT NULL REFERENCES tenants(id),
	id uuid NOT NULL,
	name text NOT NULL CHECK(length(name) BETWEEN 1 AND 120),
	PRIMARY KEY(tenant_id,id),
	UNIQUE(tenant_id,name)
);
ALTER TABLE servers ADD COLUMN farm_id uuid;
ALTER TABLE servers ADD COLUMN paused boolean NOT NULL DEFAULT false;
ALTER TABLE servers ADD CONSTRAINT server_farm_owner FOREIGN KEY(tenant_id,farm_id) REFERENCES farms(tenant_id,id);
ALTER TABLE sessions ADD COLUMN pending_notify boolean NOT NULL DEFAULT false;
CREATE TABLE audit_events (
	tenant_id uuid NOT NULL REFERENCES tenants(id),
	id bigint GENERATED ALWAYS AS IDENTITY,
	actor_id uuid REFERENCES accounts(id),
	actor_email text NOT NULL,
	action text NOT NULL,
	resource text NOT NULL,
	created_at timestamptz NOT NULL DEFAULT now(),
	PRIMARY KEY(tenant_id,id)
);
CREATE INDEX audit_by_time ON audit_events(tenant_id,created_at DESC);
ALTER TABLE memberships ENABLE ROW LEVEL SECURITY;
ALTER TABLE memberships FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON memberships USING(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid);
ALTER TABLE farms ENABLE ROW LEVEL SECURITY;
ALTER TABLE farms FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON farms USING(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid);
ALTER TABLE audit_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE audit_events FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON audit_events USING(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid);
REVOKE UPDATE, DELETE ON audit_events FROM vhmgr;
COMMIT;
