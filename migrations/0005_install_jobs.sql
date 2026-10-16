BEGIN;
ALTER TABLE servers ADD COLUMN agent_credential bytea;
ALTER TABLE access_tokens DROP CONSTRAINT access_tokens_max_clients_check;
ALTER TABLE access_tokens ADD CONSTRAINT access_tokens_max_clients_check CHECK(max_clients BETWEEN 0 AND 10000);
CREATE TABLE install_jobs (
	tenant_id uuid NOT NULL,
	id uuid NOT NULL,
	server_id uuid NOT NULL,
	actor_id uuid NOT NULL REFERENCES accounts(id),
	host text NOT NULL,
	ssh_port integer NOT NULL CHECK(ssh_port BETWEEN 1 AND 65535),
	ssh_user text NOT NULL,
	status text NOT NULL DEFAULT 'Queued' CHECK(status IN ('Queued','Running','Succeeded','Failed')),
	created_at timestamptz NOT NULL DEFAULT now(),
	started_at timestamptz,
	finished_at timestamptz,
	log text NOT NULL DEFAULT '',
	PRIMARY KEY(tenant_id,id),
	FOREIGN KEY(tenant_id,server_id) REFERENCES servers(tenant_id,id)
);
CREATE UNIQUE INDEX install_jobs_one_active ON install_jobs(tenant_id,server_id) WHERE status IN ('Queued','Running');
ALTER TABLE install_jobs ENABLE ROW LEVEL SECURITY;
ALTER TABLE install_jobs FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON install_jobs USING(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid);
COMMIT;
