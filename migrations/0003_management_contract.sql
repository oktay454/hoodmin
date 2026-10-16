BEGIN;
CREATE TABLE access_profiles (
	tenant_id uuid NOT NULL REFERENCES tenants(id),
	id uuid NOT NULL,
	name text NOT NULL CHECK(length(name) BETWEEN 1 AND 120),
	is_public boolean NOT NULL DEFAULT false,
	policy jsonb NOT NULL,
	PRIMARY KEY(tenant_id,id),
	UNIQUE(tenant_id,name)
);
ALTER TABLE access_profiles ENABLE ROW LEVEL SECURITY;
ALTER TABLE access_profiles FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON access_profiles USING(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid);
ALTER TABLE access_tokens ADD COLUMN profile_id uuid;
ALTER TABLE access_tokens ADD CONSTRAINT token_profile_owner FOREIGN KEY(tenant_id,profile_id) REFERENCES access_profiles(tenant_id,id);
ALTER TABLE access_tokens ADD COLUMN policy jsonb NOT NULL DEFAULT '{}';
ALTER TABLE access_tokens ADD COLUMN created_at timestamptz NOT NULL DEFAULT now();
ALTER TABLE access_tokens ADD COLUMN first_used_at timestamptz;
ALTER TABLE access_tokens ADD COLUMN last_used_at timestamptz;
ALTER TABLE access_tokens ADD COLUMN cycle_started_at timestamptz;
ALTER TABLE access_tokens ADD COLUMN cycle_sent bigint NOT NULL DEFAULT 0 CHECK(cycle_sent>=0);
ALTER TABLE access_tokens ADD COLUMN cycle_received bigint NOT NULL DEFAULT 0 CHECK(cycle_received>=0);
ALTER TABLE access_tokens ADD COLUMN description text NOT NULL DEFAULT '';
ALTER TABLE access_tokens ADD COLUMN group_name text NOT NULL DEFAULT '';
ALTER TABLE access_tokens ADD COLUMN customer_id text NOT NULL DEFAULT '';
ALTER TABLE access_tokens ADD COLUMN order_id text NOT NULL DEFAULT '';
ALTER TABLE access_tokens ADD COLUMN support_id text NOT NULL DEFAULT '';
ALTER TABLE sessions ADD COLUMN metadata jsonb NOT NULL DEFAULT '{}';
ALTER TABLE sessions ADD COLUMN decision text NOT NULL DEFAULT 'Ok';
ALTER TABLE sessions ADD COLUMN suppressed_by text NOT NULL DEFAULT 'None';
ALTER TABLE sessions ADD COLUMN suppressed_to text NOT NULL DEFAULT 'None';
ALTER TABLE sessions ADD COLUMN expires_at timestamptz;
ALTER TABLE sessions ADD COLUMN premium boolean NOT NULL DEFAULT true;
ALTER TABLE sessions ADD COLUMN ad_requirement text NOT NULL DEFAULT 'None';
CREATE TABLE access_devices (
	tenant_id uuid NOT NULL,
	token_id uuid NOT NULL,
	client_hash bytea NOT NULL,
	first_used_at timestamptz NOT NULL DEFAULT now(),
	last_used_at timestamptz NOT NULL DEFAULT now(),
	metadata jsonb NOT NULL DEFAULT '{}',
	PRIMARY KEY(tenant_id,token_id,client_hash),
	FOREIGN KEY(tenant_id,token_id) REFERENCES access_tokens(tenant_id,id)
);
ALTER TABLE access_devices ENABLE ROW LEVEL SECURITY;
ALTER TABLE access_devices FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON access_devices USING(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid);
CREATE TABLE acme_challenges (
	tenant_id uuid NOT NULL,
	server_id uuid NOT NULL,
	token text NOT NULL CHECK(length(token) BETWEEN 16 AND 256),
	key_authorization text NOT NULL CHECK(length(key_authorization) BETWEEN 16 AND 1024),
	expires_at timestamptz NOT NULL,
	PRIMARY KEY(tenant_id,server_id,token),
	FOREIGN KEY(tenant_id,server_id) REFERENCES servers(tenant_id,id)
);
ALTER TABLE acme_challenges ENABLE ROW LEVEL SECURITY;
ALTER TABLE acme_challenges FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON acme_challenges USING(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid);
CREATE TABLE server_samples (
	tenant_id uuid NOT NULL,
	server_id uuid NOT NULL,
	created_at timestamptz NOT NULL DEFAULT now(),
	status jsonb NOT NULL,
	FOREIGN KEY(tenant_id,server_id) REFERENCES servers(tenant_id,id)
);
CREATE INDEX server_samples_time ON server_samples(tenant_id,server_id,created_at DESC);
ALTER TABLE server_samples ENABLE ROW LEVEL SECURITY;
ALTER TABLE server_samples FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON server_samples USING(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid);
COMMIT;
