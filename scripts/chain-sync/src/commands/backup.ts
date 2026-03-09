import { closeDb, configureSchema, withDbRetry } from "../db/client.ts";
import {
  getLastSyncedBlock,
  insertBlocksAndEvents,
  insertRuntimeVersion,
  replaceBlocksAndEvents,
  upsertMetadata,
  getKnownSpecVersions,
  getBlockRange,
  getEventRange,
  type BlockRow,
  type EventRow,
} from "../db/queries.ts";
import { createRpcClient, type RpcClient, type FetchedBlock } from "../rpc/client.ts";
import { showProgress, clearProgress } from "../utils/progress.ts";
import type { ChainConfig } from "../utils/networks.ts";

interface BackupOptions {
  from?: number;
  to?: number;
  batchSize: number;
  continuous: boolean;
  verify: boolean;
}

export function parseBackupArgs(args: string[]): BackupOptions {
  const opts: BackupOptions = {
    batchSize: 100,
    continuous: false,
    verify: false,
  };

  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    switch (arg) {
      case "--from": {
        if (i + 1 >= args.length) {
          console.error("ERROR: --from requires a value");
          process.exit(1);
        }
        const val = parseInt(args[++i]!, 10);
        if (isNaN(val) || val < 0) {
          console.error("ERROR: --from requires a non-negative integer");
          process.exit(1);
        }
        opts.from = val;
        break;
      }
      case "--to": {
        if (i + 1 >= args.length) {
          console.error("ERROR: --to requires a value");
          process.exit(1);
        }
        const val = parseInt(args[++i]!, 10);
        if (isNaN(val) || val < 0) {
          console.error("ERROR: --to requires a non-negative integer");
          process.exit(1);
        }
        opts.to = val;
        break;
      }
      case "--batch-size": {
        if (i + 1 >= args.length) {
          console.error("ERROR: --batch-size requires a value");
          process.exit(1);
        }
        const val = parseInt(args[++i]!, 10);
        if (isNaN(val) || val <= 0) {
          console.error("ERROR: --batch-size requires a positive integer");
          process.exit(1);
        }
        opts.batchSize = val;
        break;
      }
      case "--continuous":
        opts.continuous = true;
        break;
      case "--verify":
        opts.verify = true;
        break;
      default:
        console.warn(`WARNING: Unknown argument: ${arg}`);
        break;
    }
  }

  // --verify and --continuous are mutually exclusive; check here before any I/O.
  if (opts.verify && opts.continuous) {
    console.error("ERROR: --verify and --continuous cannot be used together");
    process.exit(1);
  }

  return opts;
}

function toBlockRows(blocks: FetchedBlock[]): BlockRow[] {
  return blocks.map(toBlockRow);
}

function toEventRows(blocks: FetchedBlock[]): EventRow[] {
  return blocks.map(toEventRow);
}

function toBlockRow(b: FetchedBlock): BlockRow {
  return {
    block_number: b.blockNumber,
    block_hash: b.blockHash,
    parent_hash: b.parentHash,
    state_root: b.stateRoot,
    extrinsics_root: b.extrinsicsRoot,
    digest: b.digest,
    extrinsics: b.extrinsics,
    justifications: b.justifications,
    spec_version: b.specVersion,
    extrinsic_count: b.extrinsicCount,
    timestamp_ms: b.timestampMs,
  };
}

function toEventRow(b: FetchedBlock): EventRow {
  return {
    block_number: b.blockNumber,
    events_raw: b.eventsRaw,
    event_count: b.eventCount,
  };
}

function buffersEqual(a: Buffer | null, b: Buffer | null): boolean {
  if (a === null && b === null) return true;
  if (a === null || b === null) return false;
  return a.equals(b);
}

interface VerifyStats {
  checked: number;
  mismatched: number;
  missing: number;
  repaired: number;
}

async function verifyRange(
  rpc: RpcClient,
  startBlock: number,
  endBlock: number,
  batchSize: number,
  shouldStop: () => boolean,
): Promise<VerifyStats> {
  const totalBlocks = endBlock - startBlock + 1;
  const stats: VerifyStats = { checked: 0, mismatched: 0, missing: 0, repaired: 0 };
  const startTime = Date.now();
  const knownSpecs = await withDbRetry(() => getKnownSpecVersions());

  for (
    let batchStart = startBlock;
    batchStart <= endBlock;
    batchStart += batchSize
  ) {
    if (shouldStop()) break;
    const batchEnd = Math.min(batchStart + batchSize - 1, endBlock);

    // Fetch from RPC (source of truth) and PG in parallel
    const [rpcBlocks, pgBlocks, pgEvents] = await Promise.all([
      rpc.fetchBlockBatch(batchStart, batchEnd, shouldStop),
      withDbRetry(() => getBlockRange(batchStart, batchEnd)),
      withDbRetry(() => getEventRange(batchStart, batchEnd)),
    ]);

    // Index PG data by block_number
    const pgBlockMap = new Map<number, BlockRow>();
    for (const b of pgBlocks) pgBlockMap.set(b.block_number, b);
    const pgEventMap = new Map<number, EventRow>();
    for (const e of pgEvents) pgEventMap.set(e.block_number, e);

    const replaceBlocks: BlockRow[] = [];
    const replaceEvents: EventRow[] = [];

    for (const rpcBlock of rpcBlocks) {
      const pgBlock = pgBlockMap.get(rpcBlock.blockNumber);
      const pgEvent = pgEventMap.get(rpcBlock.blockNumber);

      if (!pgBlock) {
        // Missing from PG
        stats.missing++;
        replaceBlocks.push(toBlockRow(rpcBlock));
        replaceEvents.push(toEventRow(rpcBlock));
        continue;
      }

      // Compare: block_hash covers all header fields
      let mismatch = false;
      if (!rpcBlock.blockHash.equals(pgBlock.block_hash)) {
        mismatch = true;
      } else if (!rpcBlock.extrinsics.equals(pgBlock.extrinsics)) {
        mismatch = true;
      } else if (!buffersEqual(rpcBlock.justifications, pgBlock.justifications)) {
        mismatch = true;
      } else if (!pgEvent) {
        mismatch = true;
      } else if (!rpcBlock.eventsRaw.equals(pgEvent.events_raw)) {
        mismatch = true;
      } else if (rpcBlock.eventCount !== pgEvent.event_count) {
        mismatch = true;
      }

      if (mismatch) {
        console.log(`\n  Mismatch at block #${rpcBlock.blockNumber}`);
        stats.mismatched++;
        replaceBlocks.push(toBlockRow(rpcBlock));
        replaceEvents.push(toEventRow(rpcBlock));
      }
    }

    // Handle new runtime versions (same as backupRange)
    for (const block of rpcBlocks) {
      const sv = block.runtimeVersion.specVersion;
      if (!knownSpecs.has(sv)) {
        console.log(
          `\n  New runtime version: v${sv} (${block.runtimeVersion.specName}) at block #${block.blockNumber}`,
        );
        const metadataRaw = await rpc.getMetadataAtBlock(
          `0x${block.blockHash.toString("hex")}`,
        );
        await withDbRetry(() =>
          insertRuntimeVersion({
            spec_version: sv,
            spec_name: block.runtimeVersion.specName,
            impl_version: block.runtimeVersion.implVersion,
            metadata_raw: metadataRaw,
            first_block: block.blockNumber,
          }),
        );
        knownSpecs.add(sv);
      }
    }

    // DELETE + INSERT only mismatched/missing blocks
    if (replaceBlocks.length > 0) {
      await withDbRetry(() =>
        replaceBlocksAndEvents(replaceBlocks, replaceEvents),
      );
      stats.repaired += replaceBlocks.length;
    }

    stats.checked += rpcBlocks.length;
    showProgress(stats.checked, totalBlocks, startTime, "Verify");
  }

  clearProgress();
  return stats;
}

async function backupRange(
  rpc: RpcClient,
  startBlock: number,
  endBlock: number,
  batchSize: number,
  shouldStop: () => boolean,
): Promise<number> {
  const totalBlocks = endBlock - startBlock + 1;
  let synced = 0;
  const startTime = Date.now();
  // Intentionally called once outside the batch loop: the Set is kept in
  // memory and updated incrementally as new spec versions are encountered.
  // Duplicate inserts are harmless because insertRuntimeVersion uses
  // ON CONFLICT DO NOTHING, but the in-memory Set avoids the redundant DB
  // round-trips and metadata fetches.
  const knownSpecs = await withDbRetry(() => getKnownSpecVersions());

  for (
    let batchStart = startBlock;
    batchStart <= endBlock;
    batchStart += batchSize
  ) {
    if (shouldStop()) break;
    const batchEnd = Math.min(batchStart + batchSize - 1, endBlock);

    const blocks = await rpc.fetchBlockBatch(batchStart, batchEnd, shouldStop);

    // Check for new runtime versions
    for (const block of blocks) {
      const sv = block.runtimeVersion.specVersion;
      if (!knownSpecs.has(sv)) {
        console.log(
          `\n  New runtime version: v${sv} (${block.runtimeVersion.specName}) at block #${block.blockNumber}`,
        );
        const metadataRaw = await rpc.getMetadataAtBlock(
          `0x${block.blockHash.toString("hex")}`,
        );
        await withDbRetry(() =>
          insertRuntimeVersion({
            spec_version: sv,
            spec_name: block.runtimeVersion.specName,
            impl_version: block.runtimeVersion.implVersion,
            metadata_raw: metadataRaw,
            first_block: block.blockNumber,
          }),
        );
        knownSpecs.add(sv);
      }
    }

    // Insert blocks and events in a single atomic transaction
    await withDbRetry(() =>
      insertBlocksAndEvents(toBlockRows(blocks), toEventRows(blocks)),
    );

    synced += blocks.length;
    showProgress(synced, totalBlocks, startTime, "Backup");
  }

  clearProgress();
  return synced;
}

export async function runBackup(opts: BackupOptions, chain: ChainConfig): Promise<void> {
  // Graceful shutdown support
  let shutdownRequested = false;
  let onShutdown: (() => void) | null = null;
  const shouldStop = () => shutdownRequested;

  const sigintHandler = () => {
    if (!shutdownRequested) {
      console.log("\nGraceful shutdown requested. Finishing current batch...");
      shutdownRequested = true;
    }
    onShutdown?.();
  };
  process.on("SIGINT", sigintHandler);

  // Configure schema for this chain
  await configureSchema(chain.chainId);

  const rpc = createRpcClient(chain);
  const api = await rpc.connectApi();

  try {
    // Store chain info
    await withDbRetry(() => upsertMetadata("genesis_hash", api.genesisHash.toHex()));
    await withDbRetry(() => upsertMetadata("chain_name", api.runtimeChain.toString()));
    await withDbRetry(() => upsertMetadata("runtime_name", chain.runtimeName));

    // Determine range
    const finalized = await rpc.getFinalizedHead();
    const lastSynced = await withDbRetry(() => getLastSyncedBlock());

    if (opts.verify) {
      // Verify mode: default range is 0 to lastSynced (everything in DB)
      const startBlock = opts.from ?? 0;
      const endBlock = opts.to ?? (lastSynced ?? 0);

      if (startBlock > endBlock) {
        console.log("No blocks to verify.");
        return;
      }

      console.log(
        `Verify: #${startBlock} to #${endBlock} (${endBlock - startBlock + 1} blocks)`,
      );
      console.log(`Batch size: ${opts.batchSize}`);
      console.log("");

      const stats = await verifyRange(
        rpc,
        startBlock,
        endBlock,
        opts.batchSize,
        shouldStop,
      );

      console.log(`Verify complete.${shutdownRequested ? " (interrupted)" : ""}`);
      console.log(`  Checked:    ${stats.checked}`);
      console.log(`  Missing:    ${stats.missing}`);
      console.log(`  Mismatched: ${stats.mismatched}`);
      console.log(`  Repaired:   ${stats.repaired}`);
      return;
    }

    const startBlock = opts.from ?? (lastSynced != null ? lastSynced + 1 : 0);
    const endBlock = opts.to ?? finalized.number;

    if (startBlock > endBlock) {
      console.log(
        `Already synced up to #${lastSynced}. Chain finalized: #${finalized.number}. Nothing to do.`,
      );

      if (!opts.continuous) {
        // Let the finally block handle disconnectApi() and closeDb() cleanup.
        return;
      }
    } else {
      console.log(
        `Backup: #${startBlock} to #${endBlock} (${endBlock - startBlock + 1} blocks)`,
      );
      console.log(`Batch size: ${opts.batchSize}`);
      console.log("");

      const synced = await backupRange(
        rpc,
        startBlock,
        endBlock,
        opts.batchSize,
        shouldStop,
      );
      console.log(`Synced ${synced} blocks.${shutdownRequested ? " (interrupted)" : ""}`);
    }

    // Continuous mode
    if (opts.continuous && !shutdownRequested) {
      console.log("");
      console.log(
        "Entering continuous mode. Watching for new finalized blocks...",
      );

      // Mutex flag: prevents parallel backupRange calls if the subscription
      // fires again before the previous run finishes. The next finalized head
      // notification will catch up naturally.
      let isSyncing = false;
      let consecutiveErrors = 0;

      const unsub = await api.rpc.chain.subscribeFinalizedHeads(
        async (header) => {
          if (shutdownRequested) return;
          if (isSyncing) return; // Previous sync still in progress; skip this notification.
          isSyncing = true;
          try {
            const blockNum = header.number.toNumber();
            const currentLast = await withDbRetry(() => getLastSyncedBlock());

            if (currentLast != null && blockNum <= currentLast) return;

            const from = currentLast != null ? currentLast + 1 : blockNum;
            const synced = await backupRange(
              rpc,
              from,
              blockNum,
              opts.batchSize,
              shouldStop,
            );
            if (synced > 0) {
              console.log(
                `  Synced #${from}-#${blockNum} (${synced} blocks) at ${new Date().toISOString()}`,
              );
            }
            consecutiveErrors = 0;
          } catch (err) {
            consecutiveErrors++;
            console.error(
              `  ERROR in continuous sync (${consecutiveErrors} consecutive) at block ${header.number.toNumber()}: ${err}`,
            );
            if (consecutiveErrors >= 10) {
              console.error("  Too many consecutive errors. Shutting down.");
              shutdownRequested = true;
              onShutdown?.();
            }
          } finally {
            isSyncing = false;
          }
        },
      );

      // Keep alive until SIGINT
      await new Promise<void>((resolve) => {
        onShutdown = () => {
          unsub();
          resolve();
        };
      });
    }
  } finally {
    process.removeListener("SIGINT", sigintHandler);
    try { await rpc.disconnectApi(); } catch {}
    try { await closeDb(); } catch {}
  }
}
