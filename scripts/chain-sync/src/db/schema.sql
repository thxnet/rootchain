-- Chain Sync: PostgreSQL schema for blockchain backup/restore

-- Registry of all chains (always in public schema)
CREATE TABLE IF NOT EXISTS public.chains (
    chain_id     TEXT PRIMARY KEY,
    chain_type   TEXT NOT NULL,       -- "rootchain" | "leafchain"
    para_id      INTEGER,
    network      TEXT NOT NULL,
    schema_name  TEXT NOT NULL UNIQUE,
    display_name TEXT,
    runtime_name TEXT,
    binary_name  TEXT,
    created_at   TIMESTAMPTZ DEFAULT NOW()
);

-- Per-chain tables (created within the chain's schema via search_path)

-- Sync metadata (key-value store for tracking state)
CREATE TABLE IF NOT EXISTS sync_metadata (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    updated_at  TIMESTAMPTZ DEFAULT NOW()
);

-- Blocks (core table)
CREATE TABLE IF NOT EXISTS blocks (
    block_number    BIGINT PRIMARY KEY,
    block_hash      BYTEA NOT NULL,            -- H256 (32 bytes)
    parent_hash     BYTEA NOT NULL,
    state_root      BYTEA NOT NULL,
    extrinsics_root BYTEA NOT NULL,
    digest          BYTEA NOT NULL,            -- SCALE-encoded digest
    extrinsics      BYTEA NOT NULL,            -- SCALE-encoded Vec<OpaqueExtrinsic>
    justifications  BYTEA,                     -- SCALE-encoded, nullable
    spec_version    INTEGER NOT NULL,
    extrinsic_count INTEGER NOT NULL,
    timestamp_ms    BIGINT,                    -- from timestamp.set() inherent
    synced_at       TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_blocks_hash ON blocks (block_hash);

-- Events (separate table, can be large)
CREATE TABLE IF NOT EXISTS block_events (
    block_number    BIGINT PRIMARY KEY REFERENCES blocks(block_number) ON DELETE CASCADE,
    events_raw      BYTEA NOT NULL,            -- SCALE-encoded Vec<EventRecord>
    event_count     INTEGER NOT NULL
);

-- Runtime version snapshots
CREATE TABLE IF NOT EXISTS runtime_versions (
    spec_version    INTEGER PRIMARY KEY,
    spec_name       TEXT NOT NULL,
    impl_version    INTEGER NOT NULL,
    metadata_raw    BYTEA NOT NULL,            -- SCALE-encoded OpaqueMetadata
    first_block     INTEGER NOT NULL,
    synced_at       TIMESTAMPTZ DEFAULT NOW()
);
