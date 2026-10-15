-- Her sunucu tek kiracıya aittir. Birleşik dış anahtarlar kiracı sınırını korur.
BEGIN;
CREATE TABLE tenants (
	id uuid PRIMARY KEY,
	name text NOT NULL CHECK (length(name) BETWEEN 1 AND 120)
);
CREATE TABLE servers (
	tenant_id uuid NOT NULL REFERENCES tenants(id),
	id uuid NOT NULL,
	name text NOT NULL,
	credential_hash bytea NOT NULL CHECK (octet_length(credential_hash) = 32),
	configuration bytea NOT NULL,
	config_code text NOT NULL,
	last_seen timestamptz,
	info jsonb,
	status jsonb,
	PRIMARY KEY (tenant_id, id)
);
CREATE TABLE access_tokens (
	tenant_id uuid NOT NULL,
	id uuid NOT NULL,
	server_id uuid NOT NULL,
	name text NOT NULL,
	secret bytea NOT NULL,
	max_traffic bigint NOT NULL DEFAULT 0 CHECK (max_traffic >= 0),
	max_clients integer NOT NULL DEFAULT 1 CHECK (max_clients BETWEEN 1 AND 10000),
	expires_at timestamptz,
	revoked boolean NOT NULL DEFAULT false,
	sent bigint NOT NULL DEFAULT 0 CHECK (sent >= 0),
	received bigint NOT NULL DEFAULT 0 CHECK (received >= 0),
	PRIMARY KEY (tenant_id, id),
	UNIQUE (tenant_id, server_id, id),
	FOREIGN KEY (tenant_id, server_id) REFERENCES servers(tenant_id, id)
);
CREATE TABLE sessions (
	tenant_id uuid NOT NULL,
	id bigint GENERATED ALWAYS AS IDENTITY,
	server_id uuid NOT NULL,
	token_id uuid NOT NULL,
	client_hash bytea NOT NULL,
	session_key bytea NOT NULL,
	protocol_version integer NOT NULL,
	created_at timestamptz NOT NULL DEFAULT now(),
	last_seen timestamptz NOT NULL DEFAULT now(),
	closed boolean NOT NULL DEFAULT false,
	PRIMARY KEY (tenant_id, id),
	FOREIGN KEY (tenant_id, server_id, token_id) REFERENCES access_tokens(tenant_id, server_id, id)
);
CREATE INDEX sessions_active_token ON sessions(tenant_id, token_id, client_hash) WHERE NOT closed;
ALTER TABLE tenants ENABLE ROW LEVEL SECURITY;
ALTER TABLE tenants FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON tenants USING (id = nullif(current_setting('app.tenant_id', true), '')::uuid)
	WITH CHECK (id = nullif(current_setting('app.tenant_id', true), '')::uuid);
ALTER TABLE servers ENABLE ROW LEVEL SECURITY;
ALTER TABLE servers FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON servers
	USING (tenant_id = nullif(current_setting('app.tenant_id', true), '')::uuid)
	WITH CHECK (tenant_id = nullif(current_setting('app.tenant_id', true), '')::uuid);
ALTER TABLE access_tokens ENABLE ROW LEVEL SECURITY;
ALTER TABLE access_tokens FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON access_tokens
	USING (tenant_id = nullif(current_setting('app.tenant_id', true), '')::uuid)
	WITH CHECK (tenant_id = nullif(current_setting('app.tenant_id', true), '')::uuid);
ALTER TABLE sessions ENABLE ROW LEVEL SECURITY;
ALTER TABLE sessions FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON sessions
	USING (tenant_id = nullif(current_setting('app.tenant_id', true), '')::uuid)
	WITH CHECK (tenant_id = nullif(current_setting('app.tenant_id', true), '')::uuid);
COMMIT;
