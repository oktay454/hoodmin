BEGIN;
CREATE TABLE acme_orders (
	tenant_id uuid NOT NULL, id uuid NOT NULL, server_id uuid NOT NULL, domain text NOT NULL, email text NOT NULL,
	staging boolean NOT NULL DEFAULT true, enabled boolean NOT NULL DEFAULT true, version uuid NOT NULL DEFAULT gen_random_uuid(),
	status text NOT NULL DEFAULT 'Queued', message text NOT NULL DEFAULT '', last_attempt timestamptz, last_success timestamptz,
	expires_at timestamptz, next_due timestamptz, lease_until timestamptz,
	challenge_previous boolean, challenge_code text, challenge_token text,
	PRIMARY KEY(tenant_id,id), UNIQUE(tenant_id,server_id),
	FOREIGN KEY(tenant_id,server_id) REFERENCES servers(tenant_id,id)
);
ALTER TABLE acme_orders ENABLE ROW LEVEL SECURITY;
ALTER TABLE acme_orders FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON acme_orders USING(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid);
CREATE FUNCTION hoodmin_due_acme() RETURNS TABLE(tenant_id uuid,id uuid)
LANGUAGE sql SECURITY DEFINER SET search_path=pg_catalog AS $$
	SELECT a.tenant_id,a.id FROM public.acme_orders a WHERE a.enabled AND a.next_due<=now() AND (a.lease_until IS NULL OR a.lease_until<now()) ORDER BY a.next_due,a.id LIMIT 8
$$;
REVOKE ALL ON FUNCTION hoodmin_due_acme() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION hoodmin_due_acme() TO vhmgr;
COMMIT;
