BEGIN;
ALTER TABLE farms ADD COLUMN publication_origin text;
ALTER TABLE farms ADD COLUMN certificates bytea;
ALTER TABLE farms ADD COLUMN version uuid NOT NULL DEFAULT gen_random_uuid();
COMMIT;
