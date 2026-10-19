BEGIN;
CREATE TABLE external_publications (
	tenant_id uuid NOT NULL,
	id uuid NOT NULL,
	farm_id uuid NOT NULL,
	seed_id uuid NOT NULL,
	name text NOT NULL CHECK(length(name) BETWEEN 1 AND 120),
	publish_url text NOT NULL CHECK(length(publish_url) BETWEEN 8 AND 2048),
	specification bytea NOT NULL,
	version uuid NOT NULL DEFAULT gen_random_uuid(),
	enabled boolean NOT NULL DEFAULT true,
	auto_minutes integer NOT NULL DEFAULT 0 CHECK(auto_minutes=0 OR auto_minutes BETWEEN 5 AND 1440),
	status text NOT NULL DEFAULT 'Queued' CHECK(status IN ('Queued','Running','Succeeded','Failed','Disabled')),
	last_attempt timestamptz,
	last_success timestamptz,
	message text NOT NULL DEFAULT '',
	next_due timestamptz,
	lease_until timestamptz,
	PRIMARY KEY(tenant_id,id),
	FOREIGN KEY(tenant_id,farm_id) REFERENCES farms(tenant_id,id),
	FOREIGN KEY(tenant_id,seed_id) REFERENCES servers(tenant_id,id)
);
CREATE INDEX external_publications_due ON external_publications(next_due) WHERE enabled;
ALTER TABLE external_publications ENABLE ROW LEVEL SECURITY;
ALTER TABLE external_publications FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON external_publications USING(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid);
-- Scheduler sees only opaque identifiers of due jobs. Payload/credentials remain under tenant RLS.
CREATE FUNCTION hoodmin_due_publications() RETURNS TABLE(tenant_id uuid,id uuid)
LANGUAGE sql SECURITY DEFINER SET search_path=pg_catalog AS $$
	SELECT p.tenant_id,p.id FROM public.external_publications p
	WHERE p.enabled AND p.next_due<=now() AND (p.lease_until IS NULL OR p.lease_until<now())
	ORDER BY p.next_due,p.id LIMIT 16
$$;
REVOKE ALL ON FUNCTION hoodmin_due_publications() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION hoodmin_due_publications() TO hoodmin;
COMMIT;
