CREATE SCHEMA IF NOT EXISTS geecko;

CREATE TABLE IF NOT EXISTS geecko.workshops (
  id uuid PRIMARY KEY DEFAULT uuidv7(),
  name text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  CONSTRAINT workshops_name_unique UNIQUE (name)
);

CREATE TABLE IF NOT EXISTS geecko.devices (
  id uuid PRIMARY KEY DEFAULT uuidv7(),
  workshop_id uuid NOT NULL REFERENCES geecko.workshops(id) ON DELETE CASCADE,
  name text NOT NULL,
  platform text NOT NULL CHECK (platform IN ('windows', 'android')),
  device_prefix varchar(8) NOT NULL,
  token_hash bytea NOT NULL UNIQUE,
  enabled boolean NOT NULL DEFAULT true,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  last_seen_at timestamptz,
  revoked_at timestamptz,
  CONSTRAINT devices_workshop_prefix_unique UNIQUE (workshop_id, device_prefix)
);

CREATE TABLE IF NOT EXISTS geecko.sync_records (
  workshop_id uuid NOT NULL REFERENCES geecko.workshops(id) ON DELETE CASCADE,
  entity_type text NOT NULL,
  entity_id uuid NOT NULL,
  revision bigint NOT NULL CHECK (revision > 0),
  payload jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(payload) = 'object'),
  origin_device_id uuid NOT NULL REFERENCES geecko.devices(id) ON DELETE RESTRICT,
  deleted_at timestamptz,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (workshop_id, entity_type, entity_id)
);

CREATE TABLE IF NOT EXISTS geecko.sync_changes (
  cursor bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  workshop_id uuid NOT NULL REFERENCES geecko.workshops(id) ON DELETE CASCADE,
  operation_id uuid NOT NULL UNIQUE,
  entity_type text NOT NULL,
  entity_id uuid NOT NULL,
  revision bigint NOT NULL CHECK (revision > 0),
  payload jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(payload) = 'object'),
  origin_device_id uuid NOT NULL REFERENCES geecko.devices(id) ON DELETE RESTRICT,
  deleted_at timestamptz,
  changed_at timestamptz NOT NULL DEFAULT clock_timestamp()
);

CREATE INDEX IF NOT EXISTS sync_changes_workshop_cursor_idx
  ON geecko.sync_changes(workshop_id, cursor);

CREATE INDEX IF NOT EXISTS sync_records_updated_idx
  ON geecko.sync_records(workshop_id, updated_at);

CREATE TABLE IF NOT EXISTS geecko.sync_operations (
  operation_id uuid PRIMARY KEY,
  workshop_id uuid NOT NULL REFERENCES geecko.workshops(id) ON DELETE CASCADE,
  device_id uuid NOT NULL REFERENCES geecko.devices(id) ON DELETE RESTRICT,
  entity_type text NOT NULL,
  entity_id uuid NOT NULL,
  result_status text NOT NULL CHECK (result_status IN ('applied', 'conflict')),
  server_revision bigint NOT NULL CHECK (server_revision >= 0),
  change_cursor bigint REFERENCES geecko.sync_changes(cursor) ON DELETE RESTRICT,
  processed_at timestamptz NOT NULL DEFAULT clock_timestamp()
);

CREATE INDEX IF NOT EXISTS sync_operations_device_idx
  ON geecko.sync_operations(device_id, processed_at DESC);

