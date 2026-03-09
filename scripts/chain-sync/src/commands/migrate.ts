import { getDb, closeDb, configureSchema } from "../db/client.ts";
import { registerChain } from "../db/queries.ts";
import type { ChainConfig } from "../utils/networks.ts";
import { join } from "path";

const LEGACY_TABLES = ["blocks", "block_events", "runtime_versions", "sync_metadata"];

/**
 * Check if a table exists in a specific schema.
 */
async function tableExistsInSchema(schema: string, table: string): Promise<boolean> {
  const db = getDb();
  const rows = await db`
    SELECT 1 FROM information_schema.tables
    WHERE table_schema = ${schema} AND table_name = ${table}
    LIMIT 1
  `;
  return rows.length > 0;
}

/**
 * Check if a table has any rows.
 *
 * Safety note: `schema` and `table` are always hardcoded values — `schema`
 * comes from ChainConfig.chainId (a compile-time constant in networks.ts) and
 * `table` comes from the LEGACY_TABLES constant defined above. Neither is
 * derived from user input, so interpolating them into the SQL string with
 * double-quote quoting is safe here.
 */
async function tableHasData(schema: string, table: string): Promise<boolean> {
  if (!/^[a-z][a-z0-9_]*$/.test(schema) || !/^[a-z][a-z0-9_]*$/.test(table)) {
    throw new Error(`Invalid identifier: schema=${schema}, table=${table}`);
  }
  const db = getDb();
  const rows = await db.unsafe(
    `SELECT 1 FROM "${schema}"."${table}" LIMIT 1`,
  );
  return rows.length > 0;
}

/**
 * Migrate legacy public schema data into chain-specific schema.
 * Only runs for rootchain_mainnet when public.blocks exists with data.
 */
async function migrateLegacyData(chain: ChainConfig): Promise<void> {
  if (chain.chainId !== "rootchain_mainnet") return;

  const db = getDb();

  // Check if public.blocks exists and has data
  const hasPublicBlocks = await tableExistsInSchema("public", "blocks");
  if (!hasPublicBlocks) return;

  const hasData = await tableHasData("public", "blocks");
  if (!hasData) return;

  // Check if target schema already has blocks (already migrated)
  const targetHasBlocks = await tableExistsInSchema(chain.chainId, "blocks");
  if (targetHasBlocks) {
    const targetHasData = await tableHasData(chain.chainId, "blocks");
    if (targetHasData) {
      console.log("  Legacy data already migrated (target schema has data). Skipping.");
      return;
    }
  }

  console.log("  Migrating legacy data from public schema to rootchain_mainnet...");

  // Move tables from public to chain schema inside a transaction so that
  // a partial failure leaves the database in a consistent state.
  // Safety: `table` is always an element of LEGACY_TABLES (hardcoded constant)
  // and `chain.chainId` is a hardcoded constant from networks.ts — neither
  // comes from user input, so double-quote-quoted interpolation is safe here.
  await db.begin(async (tx) => {
    for (const table of LEGACY_TABLES) {
      const exists = await tableExistsInSchema("public", table);
      if (!exists) continue;

      // Drop empty target table if it exists (from schema.sql execution)
      if (targetHasBlocks) {
        await tx.unsafe(`DROP TABLE IF EXISTS "${chain.chainId}"."${table}" CASCADE`);
      }

      console.log(`    ALTER TABLE public.${table} SET SCHEMA ${chain.chainId}`);
      await tx.unsafe(`ALTER TABLE public."${table}" SET SCHEMA "${chain.chainId}"`);
    }
  });

  console.log("  Legacy data migration complete.");
}

export async function runMigrate(chain: ChainConfig): Promise<void> {
  console.log(`Running migration for ${chain.displayName} (${chain.chainId})...`);

  const db = getDb();
  const schemaPath = join(import.meta.dir, "..", "db", "schema.sql");

  try {
    // 1. Create chain-specific schema
    await configureSchema(chain.chainId);
    console.log(`  Schema "${chain.chainId}" ready.`);

    // 2. Migrate legacy data if applicable (before creating tables)
    await migrateLegacyData(chain);

    // 3. Execute schema.sql (creates public.chains + per-schema tables)
    await db.file(schemaPath);
    console.log("  Tables created/verified:");
    console.log("    - public.chains (registry)");
    console.log("    - sync_metadata");
    console.log("    - blocks");
    console.log("    - block_events");
    console.log("    - runtime_versions");

    // 4. Register chain
    await registerChain(chain);
    console.log(`  Chain registered: ${chain.chainId}`);

    console.log("Migration complete.");
  } finally {
    await closeDb();
  }
}
