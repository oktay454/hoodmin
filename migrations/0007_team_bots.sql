BEGIN;
ALTER TABLE memberships DROP CONSTRAINT memberships_role_check;
ALTER TABLE memberships ADD CONSTRAINT memberships_role_check CHECK(role IN ('owner','admin','operator','token_writer','viewer'));
CREATE TABLE bots (
	tenant_id uuid NOT NULL,
	id uuid NOT NULL,
	name text NOT NULL CHECK(length(name) BETWEEN 1 AND 120),
	credential_hash bytea NOT NULL CHECK(octet_length(credential_hash)=32),
	created_at timestamptz NOT NULL DEFAULT now(),
	expires_at timestamptz NOT NULL,
	revoked boolean NOT NULL DEFAULT false,
	PRIMARY KEY(tenant_id,id),
	FOREIGN KEY(tenant_id,id) REFERENCES memberships(tenant_id,account_id)
);
ALTER TABLE bots ENABLE ROW LEVEL SECURITY;
ALTER TABLE bots FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON bots USING(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid);
COMMIT;
