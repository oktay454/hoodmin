BEGIN;
ALTER TABLE farms ADD COLUMN server_profile_id uuid;
ALTER TABLE farms ADD CONSTRAINT farm_server_profile_tenant
	FOREIGN KEY(tenant_id,server_profile_id) REFERENCES server_profiles(tenant_id,id);
CREATE INDEX farms_server_profile ON farms(tenant_id,server_profile_id) WHERE server_profile_id IS NOT NULL;
COMMIT;
