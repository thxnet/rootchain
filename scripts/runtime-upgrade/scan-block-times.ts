/**
 * Scan block timestamps to find finalization delays / block time anomalies.
 *
 * Usage:
 *   bun run scan-block-times.ts [end_block] [range]
 *
 * If end_block is omitted, the latest block number is fetched from the chain.
 *
 * Environment variables:
 *   NETWORK      — "mainnet" | "testnet" (default: "mainnet")
 *   RPC_ENDPOINT — override the preset RPC endpoint
 */

import { ApiPromise, WsProvider } from "@polkadot/api";
import { resolveNetwork } from "./networks";

const network = resolveNetwork();

const RANGE = parseInt(process.argv[3] || "5000");

// Normal BABE block time = 6s. Flag anything above this threshold.
const ANOMALY_THRESHOLD_MS = 12_000; // 2x normal

async function main() {
  console.log(`Network: ${network.name}`);
  console.log(`Connecting to: ${network.rpcEndpoint}`);
  const provider = new WsProvider(network.rpcEndpoint);
  const api = await ApiPromise.create({ provider });

  // Resolve END_BLOCK: CLI arg or latest block from chain
  let END_BLOCK: number;
  if (process.argv[2]) {
    END_BLOCK = parseInt(process.argv[2]);
  } else {
    const latestHeader = await api.rpc.chain.getHeader();
    END_BLOCK = latestHeader.number.toNumber();
    console.log(`Using latest block: #${END_BLOCK}`);
  }

  const START_BLOCK = END_BLOCK - RANGE;

  console.log(
    `Scanning blocks #${START_BLOCK} to #${END_BLOCK} (${RANGE} blocks)`,
  );
  console.log(`Anomaly threshold: >${ANOMALY_THRESHOLD_MS / 1000}s block time`);
  console.log("");

  // Batch fetch: get timestamps for all blocks
  const BATCH_SIZE = 100;
  const timestamps: Map<number, number> = new Map();

  for (
    let batchStart = START_BLOCK;
    batchStart <= END_BLOCK;
    batchStart += BATCH_SIZE
  ) {
    const batchEnd = Math.min(batchStart + BATCH_SIZE - 1, END_BLOCK);
    const promises: Promise<void>[] = [];

    for (let blockNum = batchStart; blockNum <= batchEnd; blockNum++) {
      promises.push(
        (async () => {
          const MAX_RETRIES = 3;
          for (let attempt = 1; attempt <= MAX_RETRIES; attempt++) {
            try {
              const hash = await api.rpc.chain.getBlockHash(blockNum);
              const apiAt = await api.at(hash);
              const ts = await apiAt.query.timestamp.now();
              timestamps.set(blockNum, ts.toPrimitive() as number);
              return;
            } catch (err) {
              if (attempt === MAX_RETRIES) {
                console.warn(
                  `\n  WARNING: Failed to fetch block #${blockNum} after ${MAX_RETRIES} attempts: ${err}`,
                );
              } else {
                await new Promise((r) => setTimeout(r, 500 * attempt));
              }
            }
          }
        })(),
      );
    }

    await Promise.all(promises);

    const progress = (((batchEnd - START_BLOCK) / RANGE) * 100).toFixed(1);
    process.stdout.write(`\r  Fetched up to #${batchEnd} (${progress}%)`);
  }

  console.log("\n");

  // Analyze: find anomalies
  interface Anomaly {
    blockNum: number;
    blockTimeMs: number;
    timestamp: Date;
  }

  const anomalies: Anomaly[] = [];
  let maxBlockTime = 0;
  let maxBlockTimeBlock = 0;
  let totalBlockTime = 0;
  let blockCount = 0;

  // Collect all block times for statistics
  const blockTimes: number[] = [];

  for (let blockNum = START_BLOCK + 1; blockNum <= END_BLOCK; blockNum++) {
    const prevTs = timestamps.get(blockNum - 1);
    const currTs = timestamps.get(blockNum);

    if (prevTs === undefined || currTs === undefined) continue;

    const blockTimeMs = currTs - prevTs;
    blockTimes.push(blockTimeMs);
    totalBlockTime += blockTimeMs;
    blockCount++;

    if (blockTimeMs > maxBlockTime) {
      maxBlockTime = blockTimeMs;
      maxBlockTimeBlock = blockNum;
    }

    if (blockTimeMs > ANOMALY_THRESHOLD_MS) {
      anomalies.push({
        blockNum,
        blockTimeMs,
        timestamp: new Date(currTs),
      });
    }
  }

  // Sort block times for percentile stats
  blockTimes.sort((a, b) => a - b);

  console.log("=== Block Time Statistics ===");
  console.log(`  Blocks analyzed: ${blockCount}`);
  console.log(
    `  Average block time: ${(totalBlockTime / blockCount / 1000).toFixed(2)}s`,
  );
  console.log(
    `  Median block time: ${(blockTimes[Math.floor(blockTimes.length / 2)]! / 1000).toFixed(2)}s`,
  );
  console.log(
    `  P95 block time: ${(blockTimes[Math.floor(blockTimes.length * 0.95)]! / 1000).toFixed(2)}s`,
  );
  console.log(
    `  P99 block time: ${(blockTimes[Math.floor(blockTimes.length * 0.99)]! / 1000).toFixed(2)}s`,
  );
  console.log(
    `  Max block time: ${(maxBlockTime / 1000).toFixed(2)}s at block #${maxBlockTimeBlock}`,
  );
  console.log("");

  // Group anomalies into "incidents" (consecutive or close anomalies)
  console.log(
    `=== Anomalies (block time > ${ANOMALY_THRESHOLD_MS / 1000}s) ===`,
  );
  console.log(`  Total anomalous blocks: ${anomalies.length}`);
  console.log("");

  if (anomalies.length === 0) {
    console.log("  No anomalies found.");
    await api.disconnect();
    return;
  }

  // Group into incidents: anomalies within 50 blocks of each other
  interface Incident {
    startBlock: number;
    endBlock: number;
    anomalies: Anomaly[];
    peakBlockTimeMs: number;
    totalDelayMs: number;
  }

  const incidents: Incident[] = [];
  let currentIncident: Incident | null = null;

  for (const anomaly of anomalies) {
    if (!currentIncident || anomaly.blockNum - currentIncident.endBlock > 50) {
      currentIncident = {
        startBlock: anomaly.blockNum,
        endBlock: anomaly.blockNum,
        anomalies: [anomaly],
        peakBlockTimeMs: anomaly.blockTimeMs,
        totalDelayMs: anomaly.blockTimeMs - 6000,
      };
      incidents.push(currentIncident);
    } else {
      currentIncident.endBlock = anomaly.blockNum;
      currentIncident.anomalies.push(anomaly);
      if (anomaly.blockTimeMs > currentIncident.peakBlockTimeMs) {
        currentIncident.peakBlockTimeMs = anomaly.blockTimeMs;
      }
      currentIncident.totalDelayMs += anomaly.blockTimeMs - 6000;
    }
  }

  console.log(`  Grouped into ${incidents.length} incident(s):\n`);

  for (let i = 0; i < incidents.length; i++) {
    const inc = incidents[i]!;
    const startTs = timestamps.get(inc.startBlock);
    const endTs = timestamps.get(inc.endBlock);
    console.log(`  --- Incident #${i + 1} ---`);
    console.log(
      `  Block range: #${inc.startBlock} - #${inc.endBlock} (${inc.endBlock - inc.startBlock + 1} blocks)`,
    );
    console.log(
      `  Time range: ${startTs ? new Date(startTs).toISOString() : "?"} - ${endTs ? new Date(endTs).toISOString() : "?"}`,
    );
    console.log(`  Anomalous blocks: ${inc.anomalies.length}`);
    console.log(
      `  Peak block time: ${(inc.peakBlockTimeMs / 1000).toFixed(1)}s`,
    );
    console.log(
      `  Total excess delay: ${(inc.totalDelayMs / 1000).toFixed(1)}s`,
    );
    console.log(`  Top 5 slowest blocks:`);

    const sorted = [...inc.anomalies]
      .sort((a, b) => b.blockTimeMs - a.blockTimeMs)
      .slice(0, 5);
    for (const a of sorted) {
      console.log(
        `    #${a.blockNum}: ${(a.blockTimeMs / 1000).toFixed(1)}s (${a.timestamp.toISOString()})`,
      );
    }
    console.log("");
  }

  await api.disconnect();
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
