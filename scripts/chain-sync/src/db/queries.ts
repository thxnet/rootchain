import { getDb } from "./client.ts";
import type { ChainConfig } from "../utils/networks.ts";

export interface BlockRow {
  block_number: number;
  block_hash: Buffer;
  parent_hash: Buffer;
  state_root: Buffer;
  extrinsics_root: Buffer;
  digest: Buffer;
  extrinsics: Buffer;
  justifications: Buffer | null;
  spec_version: number;
  extrinsic_count: number;
  timestamp_ms: number | null;
}

export interface EventRow {
  block_number: number;
  events_raw: Buffer;
  event_count: number;
}

export interface RuntimeVersionRow {
  spec_version: number;
  spec_name: string;
  impl_version: number;
  metadata_raw: Buffer;
  first_block: number;
}

function rowToBlockRow(r: Record<string, unknown>): BlockRow {
  return {
    block_number: Number(r["block_number"]),
    block_hash: Buffer.isBuffer(r["block_hash"])
      ? r["block_hash"]
      : Buffer.from(r["block_hash"] as Uint8Array),
    parent_hash: Buffer.isBuffer(r["parent_hash"])
      ? r["parent_hash"]
      : Buffer.from(r["parent_hash"] as Uint8Array),
    state_root: Buffer.isBuffer(r["state_root"])
      ? r["state_root"]
      : Buffer.from(r["state_root"] as Uint8Array),
    extrinsics_root: Buffer.isBuffer(r["extrinsics_root"])
      ? r["extrinsics_root"]
      : Buffer.from(r["extrinsics_root"] as Uint8Array),
    digest: Buffer.isBuffer(r["digest"])
      ? r["digest"]
      : Buffer.from(r["digest"] as Uint8Array),
    extrinsics: Buffer.isBuffer(r["extrinsics"])
      ? r["extrinsics"]
      : Buffer.from(r["extrinsics"] as Uint8Array),
    justifications:
      r["justifications"] == null
        ? null
        : Buffer.isBuffer(r["justifications"])
          ? r["justifications"]
          : Buffer.from(r["justifications"] as Uint8Array),
    spec_version: Number(r["spec_version"]),
    extrinsic_count: Number(r["extrinsic_count"]),
    timestamp_ms:
      r["timestamp_ms"] == null ? null : Number(r["timestamp_ms"]),
  };
}

export async function getLastSyncedBlock(): Promise<number | null> {
  const db = getDb();
  const rows = await db`SELECT MAX(block_number) as max_block FROM blocks`;
  const val = rows[0]?.max_block;
  return val != null ? Number(val) : null;
}

export async function getSyncedBlockCount(): Promise<number> {
  const db = getDb();
  const rows = await db`SELECT COUNT(*) as cnt FROM blocks`;
  return Number(rows[0]?.cnt ?? 0);
}

export async function insertBlocksAndEvents(
  blocks: BlockRow[],
  events: EventRow[],
): Promise<void> {
  if (blocks.length === 0) return;
  const db = getDb();

  await db.begin(async (tx) => {
    await tx`INSERT INTO blocks ${tx(blocks)} ON CONFLICT (block_number) DO NOTHING`;
    if (events.length > 0) {
      await tx`INSERT INTO block_events ${tx(events)} ON CONFLICT (block_number) DO NOTHING`;
    }
  });
}

export async function insertRuntimeVersion(
  rv: RuntimeVersionRow,
): Promise<void> {
  const db = getDb();
  await db`
    INSERT INTO runtime_versions ${db(rv)}
    ON CONFLICT (spec_version) DO NOTHING
  `;
}

export async function getBlockRange(
  from: number,
  to: number,
): Promise<BlockRow[]> {
  const db = getDb();
  const rows = await db`
    SELECT block_number, block_hash, parent_hash, state_root, extrinsics_root,
           digest, extrinsics, justifications, spec_version, extrinsic_count, timestamp_ms
    FROM blocks
    WHERE block_number >= ${from} AND block_number <= ${to}
    ORDER BY block_number ASC
  `;
  return (rows as unknown as Record<string, unknown>[]).map(rowToBlockRow);
}

export async function getEventRange(
  from: number,
  to: number,
): Promise<EventRow[]> {
  const db = getDb();
  const rows = await db`
    SELECT block_number, events_raw, event_count
    FROM block_events
    WHERE block_number >= ${from} AND block_number <= ${to}
    ORDER BY block_number ASC
  `;
  return (rows as unknown as Record<string, unknown>[]).map((r) => ({
    block_number: Number(r["block_number"]),
    events_raw: Buffer.isBuffer(r["events_raw"])
      ? r["events_raw"]
      : Buffer.from(r["events_raw"] as Uint8Array),
    event_count: Number(r["event_count"]),
  }));
}

export async function replaceBlocksAndEvents(
  blocks: BlockRow[],
  events: EventRow[],
): Promise<void> {
  if (blocks.length === 0) return;
  const db = getDb();
  const blockNumbers = blocks.map((b) => b.block_number);

  await db.begin(async (tx) => {
    // Delete associated data first (FK: block_events → blocks, no CASCADE)
    await tx`DELETE FROM block_events WHERE block_number IN ${tx(blockNumbers)}`;
    await tx`DELETE FROM blocks WHERE block_number IN ${tx(blockNumbers)}`;
    // Re-insert clean data
    await tx`INSERT INTO blocks ${tx(blocks)}`;
    if (events.length > 0) {
      await tx`INSERT INTO block_events ${tx(events)}`;
    }
  });
}

export async function getGaps(): Promise<
  Array<{ gap_start: number; gap_end: number }>
> {
  const db = getDb();
  const rows = await db`
    WITH gaps AS (
      SELECT block_number,
             LEAD(block_number) OVER (ORDER BY block_number) AS next_block
      FROM blocks
    )
    SELECT block_number + 1 AS gap_start,
           next_block - 1 AS gap_end
    FROM gaps
    WHERE next_block - block_number > 1
    ORDER BY gap_start
    LIMIT 100
  `;
  return (rows as unknown as Array<{ gap_start: unknown; gap_end: unknown }>).map(
    (r) => ({
      gap_start: Number(r.gap_start),
      gap_end: Number(r.gap_end),
    }),
  );
}

export async function upsertMetadata(
  key: string,
  value: string,
): Promise<void> {
  const db = getDb();
  await db`
    INSERT INTO sync_metadata (key, value, updated_at)
    VALUES (${key}, ${value}, NOW())
    ON CONFLICT (key) DO UPDATE SET value = ${value}, updated_at = NOW()
  `;
}

export async function getMetadata(key: string): Promise<string | null> {
  const db = getDb();
  const rows = await db`SELECT value FROM sync_metadata WHERE key = ${key}`;
  return (rows[0]?.value as string) ?? null;
}

export async function getKnownSpecVersions(): Promise<Set<number>> {
  const db = getDb();
  const rows = await db`SELECT spec_version FROM runtime_versions`;
  return new Set(
    (rows as unknown as Array<{ spec_version: unknown }>).map((r) =>
      Number(r.spec_version),
    ),
  );
}

/**
 * Check if the blocks table exists in the given schema.
 */
export async function checkTablesExist(schemaName: string): Promise<boolean> {
  const db = getDb();
  const rows = await db`
    SELECT 1 FROM information_schema.tables
    WHERE table_name = 'blocks'
      AND table_schema = ${schemaName}
    LIMIT 1
  `;
  return rows.length > 0;
}

export interface RuntimeVersionStatusRow {
  spec_version: number;
  spec_name: string;
  first_block: number;
}

export async function getRuntimeVersions(): Promise<RuntimeVersionStatusRow[]> {
  const db = getDb();
  const rows = await db`
    SELECT spec_version, spec_name, first_block FROM runtime_versions
    ORDER BY spec_version ASC
  `;
  return (
    rows as unknown as Array<{
      spec_version: unknown;
      spec_name: unknown;
      first_block: unknown;
    }>
  ).map((r) => ({
    spec_version: Number(r.spec_version),
    spec_name: String(r.spec_name),
    first_block: Number(r.first_block),
  }));
}

// --- Chain registry (always in public schema) ---

export interface ChainRegistryEntry {
  chain_id: string;
  chain_type: string;
  para_id: number | null;
  network: string;
  schema_name: string;
  display_name: string | null;
  runtime_name: string | null;
  binary_name: string | null;
}

export async function registerChain(chain: ChainConfig): Promise<void> {
  const db = getDb();
  const entry = {
    chain_id: chain.chainId,
    chain_type: chain.chainType,
    para_id: chain.paraId ?? null,
    network: chain.network,
    schema_name: chain.chainId,
    display_name: chain.displayName,
    runtime_name: chain.runtimeName,
    binary_name: chain.binaryName,
  };
  await db`
    INSERT INTO public.chains ${db(entry)}
    ON CONFLICT (chain_id) DO UPDATE SET
      chain_type = EXCLUDED.chain_type,
      para_id = EXCLUDED.para_id,
      network = EXCLUDED.network,
      schema_name = EXCLUDED.schema_name,
      display_name = EXCLUDED.display_name,
      runtime_name = EXCLUDED.runtime_name,
      binary_name = EXCLUDED.binary_name
  `;
}

export async function listChains(): Promise<ChainRegistryEntry[]> {
  const db = getDb();
  const rows = await db`
    SELECT chain_id, chain_type, para_id, network, schema_name,
           display_name, runtime_name, binary_name
    FROM public.chains
    ORDER BY chain_type, para_id NULLS FIRST, network
  `;
  return (rows as unknown as Record<string, unknown>[]).map((r) => ({
    chain_id: String(r["chain_id"]),
    chain_type: String(r["chain_type"]),
    para_id: r["para_id"] == null ? null : Number(r["para_id"]),
    network: String(r["network"]),
    schema_name: String(r["schema_name"]),
    display_name: r["display_name"] == null ? null : String(r["display_name"]),
    runtime_name: r["runtime_name"] == null ? null : String(r["runtime_name"]),
    binary_name: r["binary_name"] == null ? null : String(r["binary_name"]),
  }));
}
