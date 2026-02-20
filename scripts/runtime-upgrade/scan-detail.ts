/**
 * Detailed scan around a specific block — show every block's time.
 *
 * Usage:
 *   bun run scan-detail.ts <center_block> [range]
 *
 * Example:
 *   bun run scan-detail.ts 14206274 50
 *
 * Environment variables:
 *   NETWORK      — "mainnet" | "testnet" (default: "mainnet")
 *   RPC_ENDPOINT — override the preset RPC endpoint
 */

import { ApiPromise, WsProvider } from "@polkadot/api";
import { resolveNetwork } from "./networks";

const network = resolveNetwork();

async function main() {
  const centerBlock = parseInt(process.argv[2] || "");
  const range = parseInt(process.argv[3] || "30");

  if (isNaN(centerBlock)) {
    console.error("Usage: bun run scan-detail.ts <center_block> [range]");
    console.error("");
    console.error("  center_block  Block number to scan around (required)");
    console.error(
      "  range         Number of blocks before and after center (default: 30)",
    );
    console.error("");
    console.error("Environment variables:");
    console.error('  NETWORK       "mainnet" | "testnet" (default: "mainnet")');
    console.error("  RPC_ENDPOINT  Override the preset RPC endpoint");
    process.exit(1);
  }

  const start = centerBlock - range;
  const end = centerBlock + range;

  console.log(`Network: ${network.name}`);
  console.log(`Connecting to: ${network.rpcEndpoint}`);
  const provider = new WsProvider(network.rpcEndpoint);
  const api = await ApiPromise.create({ provider });

  console.log(`\n=== Blocks #${start} - #${end} (center: #${centerBlock}) ===`);
  console.log("Block      | Block Time | Timestamp (UTC)");
  console.log("-----------|------------|----------------------------");

  let prevTs: number | null = null;

  for (let blockNum = start; blockNum <= end; blockNum++) {
    const hash = await api.rpc.chain.getBlockHash(blockNum);
    const apiAt = await api.at(hash);
    const ts = (await apiAt.query.timestamp.now()).toPrimitive() as number;

    let blockTimeStr = "     -    ";
    if (prevTs !== null) {
      const diff = (ts - prevTs) / 1000;
      const flag = diff > 12 ? " <<<" : "";
      blockTimeStr = `${diff.toFixed(1).padStart(8)}s${flag}`;
    }

    const dateStr = new Date(ts)
      .toISOString()
      .replace("T", " ")
      .replace(".000Z", "");
    console.log(`#${blockNum} | ${blockTimeStr} | ${dateStr}`);

    prevTs = ts;
  }

  await api.disconnect();
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
