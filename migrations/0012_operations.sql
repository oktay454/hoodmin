BEGIN;
CREATE TABLE vault_state (
	id boolean PRIMARY KEY DEFAULT true CHECK(id), active_key_id text CHECK(active_key_id ~ '^[0-9a-f]{32}$'),
	rotated_at timestamptz
);
INSERT INTO vault_state(id) VALUES(true);
CREATE FUNCTION hoodmin_vault_tenants() RETURNS TABLE(id uuid)
LANGUAGE sql SECURITY DEFINER SET search_path=pg_catalog SET row_security=off AS $$ SELECT t.id FROM public.tenants t ORDER BY t.id $$;
REVOKE ALL ON FUNCTION hoodmin_vault_tenants() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION hoodmin_vault_tenants() TO hoodmin;
-- Maintenance must fail rather than silently omit FORCE-RLS tenant rows.
ALTER FUNCTION hoodmin_due_acme() SET row_security=off;
ALTER FUNCTION hoodmin_due_publications() SET row_security=off;
COMMIT;
