import { closeDb, configureSchema } from "../db/client.ts";
import {
  getLastSyncedBlock,
  getSyncedBlockCount,
  getGaps,
  getMetadata,
  checkTablesExist,
  getRuntimeVersions,
  listChains,
} from "../db/queries.ts";
import { createRpcClient } from "../rpc/client.ts";
import type { ChainConfig } from "../utils/networks.ts";
import { CHAINS } from "../utils/networks.ts";

async function printChainStatus(chain: ChainConfig, compact: boolean): Promise<void> {
  await configureSchema(chain.chainId);

  const tablesExist = await checkTablesExist(chain.chainId);
  if (!tablesExist) {
    if (compact) {
      console.log(
        `  ${chain.displayName.padEnd(30)} | not initialized`,
      );
    } else {
      console.log("Database not initialized. Run 'migrate' first.");
    }
    return;
  }

  const lastBlock = await getLastSyncedBlock();
  const blockCount = await getSyncedBlockCount();
  const genesisHash = await getMetadata("genesis_hash");
  const chainName = await getMetadata("chain_name");

  if (compact) {
    // Single-line summary for --chain all mode
    const blocksStr = blockCount.toLocaleString().padStart(12);
    const highestStr = lastBlock != null ? `#${lastBlock.toLocaleString()}` : "none";
    console.log(
      `  ${chain.displayName.padEnd(30)} | ${blocksStr} blocks | highest: ${highestStr.padStart(15)}`,
    );
    return;
  }

  // Detailed single-chain status
  console.log("=== Chain Sync Status ===");
  console.log("");
  console.log(`  Chain: ${chain.displayName} (${chain.chainId})`);
  if (chain.chainType === "leafchain") {
    console.log(`  Type: leafchain (paraId: ${chain.paraId})`);
  }
  if (chainName) console.log(`  On-chain name: ${chainName}`);
  if (genesisHash) console.log(`  Genesis: ${genesisHash}`);
  console.log(`  Synced blocks: ${blockCount.toLocaleString()}`);
  console.log(
    `  Highest block: ${lastBlock != null ? `#${lastBlock.toLocaleString()}` : "none"}`,
  );

  // Try connecting to chain for live comparison
  let rpc: ReturnType<typeof createRpcClient> | undefined;
  try {
    rpc = createRpcClient(chain);
    const finalized = await rpc.getFinalizedHead();
    console.log(
      `  Chain finalized: #${finalized.number.toLocaleString()}`,
    );

    if (lastBlock != null) {
      const behind = finalized.number - lastBlock;
      console.log(`  Behind: ${behind.toLocaleString()} blocks`);
    }
  } catch {
    console.log("  (Could not connect to chain for live status)");
  } finally {
    try { await rpc?.disconnectApi(); } catch {}
  }

  // Gap detection
  if (blockCount > 0) {
    console.log("");
    const gaps = await getGaps();
    if (gaps.length === 0) {
      console.log("  Gaps: none (continuous)");
    } else {
      console.log(`  Gaps detected: ${gaps.length}`);
      for (const gap of gaps.slice(0, 10)) {
        console.log(`    #${gap.gap_start} - #${gap.gap_end}`);
      }
      if (gaps.length > 10) {
        console.log(`    ... and ${gaps.length - 10} more`);
      }
    }
  }

  // Runtime versions
  const versions = await getRuntimeVersions();
  if (versions.length > 0) {
    console.log("");
    console.log("  Runtime versions:");
    for (const v of versions) {
      console.log(
        `    v${v.spec_version} (${v.spec_name}) from block #${v.first_block}`,
      );
    }
  }
}

export async function runStatus(chain: ChainConfig): Promise<void> {
  try {
    await printChainStatus(chain, false);
  } finally {
    await closeDb();
  }
}

export async function runStatusAll(): Promise<void> {
  try {
    // Try to read from the registry
    let registeredChains: Awaited<ReturnType<typeof listChains>>;
    try {
      registeredChains = await listChains();
    } catch {
      // Registry may not exist yet
      registeredChains = [];
    }

    if (registeredChains.length === 0) {
      console.log("No chains registered. Run 'migrate' for each chain first.");
      console.log("");
      console.log("Available chains:");
      for (const c of CHAINS) {
        const spec =
          c.chainType === "leafchain"
            ? `${c.chainType}:${c.paraId}:${c.network}`
            : `${c.chainType}:${c.network}`;
        console.log(`  ${c.displayName.padEnd(30)} --chain ${spec}`);
      }
      return;
    }

    console.log("=== All Chains Status ===");
    console.log("");

    for (const entry of registeredChains) {
      // Find the matching ChainConfig
      const chain = CHAINS.find((c) => c.chainId === entry.chain_id);
      if (chain) {
        await printChainStatus(chain, true);
      } else {
        console.log(
          `  ${(entry.display_name ?? entry.chain_id).padEnd(30)} | (config not found)`,
        );
      }
    }

    console.log("");
  } finally {
    await closeDb();
  }
}
