import { closeDb, configureSchema, withDbRetry } from "../db/client.ts";
import {
  getBlockRange,
  getLastSyncedBlock,
  getMetadata,
} from "../db/queries.ts";
import { encodeBlockForImport } from "../utils/scale.ts";
import { showProgress, clearProgress } from "../utils/progress.ts";
import type { ChainConfig } from "../utils/networks.ts";
import { CHAINS } from "../utils/networks.ts";

interface RestoreOptions {
  output?: string;
  from?: number;
  to?: number;
}

export function parseRestoreArgs(args: string[]): RestoreOptions {
  const opts: RestoreOptions = {};

  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    switch (arg) {
      case "--output":
      case "-o": {
        if (i + 1 >= args.length) {
          console.error("ERROR: --output requires a value");
          process.exit(1);
        }
        opts.output = args[++i]!;
        break;
      }
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
        if (isNaN(val)) {
          console.error("ERROR: --to requires a valid integer");
          process.exit(1);
        }
        opts.to = val;
        break;
      }
      default:
        console.warn(`WARNING: Unknown argument: ${arg}`);
        break;
    }
  }

  return opts;
}

function printRootchainRestoreGuide(outputFile: string, chainName: string): void {
  console.log("To import into a new node:");
  console.log(
    `  polkadot import-blocks --chain ${chainName} --pruning archive-canonical --input ${outputFile}`,
  );
  console.log("");
  console.log(
    "WARNING: If restoring an archive node, you MUST set --pruning=archive or",
  );
  console.log(
    "  --pruning=archive-canonical on the target. Default pruning (256) will",
  );
  console.log(
    "  discard historical state during import, defeating the purpose of backup.",
  );
}

function printLeafchainRestoreGuide(
  outputFile: string,
  chain: ChainConfig,
): void {
  const rootchain = CHAINS.find(
    (c) => c.chainType === "rootchain" && c.network === chain.network,
  );
  const relayWs = rootchain?.rpcEndpoints[0] ?? "wss://node.mainnet.thxnet.org/archive-001/ws";

  console.log("=== Path A: External relay chain (recommended, fastest) ===");
  console.log("Only restore parachain blocks; relay chain data via RPC:");
  console.log("");
  console.log("  # 1. Import parachain blocks");
  console.log(
    `  ${chain.binaryName} import-blocks --chain ${chain.runtimeName} \\`,
  );
  console.log("    --pruning archive-canonical \\");
  console.log(`    --input ${outputFile}`);
  console.log("");
  console.log("  # 2. Start with external relay chain (no embedded relay sync needed)");
  console.log(
    `  ${chain.binaryName} --chain ${chain.runtimeName} \\`,
  );
  console.log(`    --relay-chain-rpc-url ${relayWs}`);
  console.log("");

  console.log("=== Path B: Embedded relay chain (fully self-contained) ===");
  console.log("Restore both rootchain + parachain blocks:");
  console.log("");
  console.log("  # 1. Restore relay chain into leafchain's embedded directory");
  console.log(
    `  polkadot import-blocks --chain ${rootchain?.runtimeName ?? "thxnet"} \\`,
  );
  console.log("    --base-path /data/polkadot \\");
  console.log("    --pruning archive-canonical \\");
  console.log("    --input rootchain-blocks.bin");
  console.log("");
  console.log("  # 2. Restore parachain blocks");
  console.log(
    `  ${chain.binaryName} import-blocks --chain ${chain.runtimeName} \\`,
  );
  console.log("    --base-path /data \\");
  console.log("    --pruning archive-canonical \\");
  console.log(`    --input ${outputFile}`);
  console.log("");
  console.log("  # 3. Start with embedded relay chain");
  console.log(
    `  ${chain.binaryName} --chain ${chain.runtimeName} --base-path /data \\`,
  );
  console.log(`    -- --chain ${rootchain?.runtimeName ?? "thxnet"}`);
  console.log("");
  console.log(
    `  Note: rootchain-blocks.bin can be restored from the same PostgreSQL's`,
  );
  console.log(
    `  rootchain_${chain.network} schema.`,
  );
  console.log("");

  console.log(
    "WARNING: If restoring an archive node, you MUST set --pruning=archive or",
  );
  console.log(
    "  --pruning=archive-canonical on the target. Default pruning (256) will",
  );
  console.log(
    "  discard historical state during import, defeating the purpose of backup.",
  );
}

export async function runRestore(opts: RestoreOptions, chain: ChainConfig): Promise<void> {
  // Configure schema for this chain
  await configureSchema(chain.chainId);

  try {
    const lastBlock = await withDbRetry(() => getLastSyncedBlock());
    if (lastBlock == null) {
      console.error("No blocks in database. Run backup first.");
      process.exit(1);
    }

    const fromBlock = opts.from ?? 0;
    const toBlock = opts.to ?? lastBlock;
    const outputFile = opts.output ?? `${chain.chainId}-blocks.bin`;

    console.log(`Restore: exporting blocks #${fromBlock} to #${toBlock}`);
    console.log(`Chain: ${chain.displayName} (${chain.chainId})`);
    console.log(`Output: ${outputFile}`);
    console.log("");

    const file = Bun.file(outputFile);
    const writer = file.writer();

    try {
      const QUERY_BATCH = 1000;
      let exported = 0;
      const totalBlocks = toBlock - fromBlock + 1;
      const startTime = Date.now();

      for (
        let batchStart = fromBlock;
        batchStart <= toBlock;
        batchStart += QUERY_BATCH
      ) {
        const batchEnd = Math.min(batchStart + QUERY_BATCH - 1, toBlock);
        const blocks = await withDbRetry(() => getBlockRange(batchStart, batchEnd));

        for (const block of blocks) {
          const encoded = encodeBlockForImport(block);
          writer.write(encoded);
          exported++;
        }

        await writer.flush();
        showProgress(exported, totalBlocks, startTime, "Export");
      }

      clearProgress();
      await writer.flush();
      await writer.end();

      const elapsed = ((Date.now() - startTime) / 1000).toFixed(1);
      console.log(`Exported ${exported} blocks in ${elapsed}s`);
      console.log(`Output file: ${outputFile}`);
      console.log("");

      // Print restore instructions based on chain type
      if (chain.chainType === "leafchain") {
        printLeafchainRestoreGuide(outputFile, chain);
      } else {
        const chainName = (await withDbRetry(() => getMetadata("runtime_name"))) ?? chain.runtimeName;
        printRootchainRestoreGuide(outputFile, chainName);
      }
    } catch (err) {
      try { writer.end(); } catch {}
      throw err;
    }
  } finally {
    await closeDb();
  }
}
