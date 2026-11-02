BEGIN;
CREATE TABLE notification_channels (
	tenant_id uuid NOT NULL REFERENCES tenants(id), id uuid NOT NULL, name text NOT NULL,
	specification bytea NOT NULL, enabled boolean NOT NULL DEFAULT true, version uuid NOT NULL,
	PRIMARY KEY(tenant_id,id)
);
CREATE TABLE notification_states (
	tenant_id uuid NOT NULL REFERENCES tenants(id), resource text NOT NULL, state text NOT NULL,
	PRIMARY KEY(tenant_id,resource)
);
CREATE TABLE notification_deliveries (
	tenant_id uuid NOT NULL, id uuid NOT NULL, channel_id uuid NOT NULL, event jsonb NOT NULL,
	attempts integer NOT NULL DEFAULT 0, status text NOT NULL DEFAULT 'Queued', message text NOT NULL DEFAULT '',
	created_at timestamptz NOT NULL DEFAULT now(), next_due timestamptz NOT NULL DEFAULT now(), lease_until timestamptz,
	PRIMARY KEY(tenant_id,id), FOREIGN KEY(tenant_id,channel_id) REFERENCES notification_channels(tenant_id,id)
);
CREATE INDEX notification_due ON notification_deliveries(tenant_id,next_due) WHERE status IN ('Queued','Retry');
ALTER TABLE notification_channels ENABLE ROW LEVEL SECURITY;
ALTER TABLE notification_channels FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON notification_channels USING(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid);
ALTER TABLE notification_states ENABLE ROW LEVEL SECURITY;
ALTER TABLE notification_states FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON notification_states USING(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid);
ALTER TABLE notification_deliveries ENABLE ROW LEVEL SECURITY;
ALTER TABLE notification_deliveries FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON notification_deliveries USING(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid);
COMMIT;
