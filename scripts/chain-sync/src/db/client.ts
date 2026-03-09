import { SQL } from "bun";

let _db: InstanceType<typeof SQL> | null = null;
let _currentSchema: string | null = null;

function createSql(): InstanceType<typeof SQL> {
  const url = process.env.PG_URL;
  if (!url) {
    console.error(
      "ERROR: PG_URL environment variable is required.\n" +
        "Example: PG_URL=postgresql://user:password@host:5432/dbname",
    );
    process.exit(1);
  }

  // max: 1 — single connection ensures SET search_path persists across all
  // queries. This is a CLI tool (not a web server), so pooling is unnecessary
  // and would cause schema isolation bugs (different connections could have
  // different search_path settings).
  return new SQL(url, {
    max: 1,
    idleTimeout: 30,
    connectionTimeout: 30,
    onclose: (_client: unknown, err: unknown) => {
      if (err) {
        console.warn(`  DB connection closed unexpectedly: ${err}`);
      }
      _db = null;
    },
  });
}

export function getDb(): InstanceType<typeof SQL> {
  if (!_db) {
    _db = createSql();
  }
  return _db;
}

export async function closeDb(): Promise<void> {
  if (_db) {
    const db = _db;
    _db = null;
    await db.close();
  }
}

/**
 * Create schema if not exists and set search_path for all subsequent queries.
 * The search_path includes public so public.chains is always accessible.
 */
export async function configureSchema(schemaName: string): Promise<void> {
  if (!/^[a-z][a-z0-9_]*$/.test(schemaName)) {
    throw new Error(`Invalid schema name: ${schemaName}`);
  }
  _currentSchema = schemaName;
  const db = getDb();
  await db.unsafe(`CREATE SCHEMA IF NOT EXISTS "${schemaName}"`);
  await db.unsafe(`SET search_path TO "${schemaName}", public`);
}

/**
 * Re-apply SET search_path on a fresh connection after reconnect.
 */
async function ensureSchema(): Promise<void> {
  if (_currentSchema) {
    if (!/^[a-z][a-z0-9_]*$/.test(_currentSchema)) {
      throw new Error(`Invalid schema name: ${_currentSchema}`);
    }
    const db = getDb();
    await db.unsafe(`SET search_path TO "${_currentSchema}", public`);
  }
}

const RETRIABLE_CODES = new Set([
  "ERR_POSTGRES_CONNECTION_CLOSED",
  "ERR_POSTGRES_CONNECTION_TIMEOUT",
]);

/**
 * Execute a DB operation with automatic reconnect on transient connection errors.
 * On failure: nulls the dead connection, waits with backoff, re-applies search_path, retries.
 */
export async function withDbRetry<T>(
  fn: () => Promise<T>,
  maxRetries = 5,
): Promise<T> {
  for (let attempt = 1; ; attempt++) {
    try {
      return await fn();
    } catch (err: any) {
      const isRetriable = err?.code && RETRIABLE_CODES.has(err.code);
      if (!isRetriable || attempt >= maxRetries) throw err;

      console.warn(
        `  DB connection error (attempt ${attempt}/${maxRetries}): ${err.message}. Reconnecting...`,
      );
      // Eagerly null out the dead connection here as a synchronous safety net.
      // The onclose callback also nulls _db, but it fires asynchronously and
      // may not have run yet — so we do it here too to ensure getDb() creates
      // a fresh connection on the very next call inside ensureSchema().
      _db = null;
      await new Promise((r) =>
        setTimeout(r, 1000 * attempt + Math.random() * 1000),
      );
      await ensureSchema();
    }
  }
}
