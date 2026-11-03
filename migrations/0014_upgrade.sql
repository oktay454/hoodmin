BEGIN;
-- Read-only migration metadata permits readiness checks, never schema changes.
GRANT SELECT ON _hoodmin_migrations TO hoodmin;
COMMIT;
