#!/usr/bin/env bun
/**
 * Verify that our SCALE reconstruction in scale.ts produces
 * byte-exact output matching the original SignedBlock from RPC.
 *
 * Usage: bun run src/verify-encoding.ts [--chain CHAIN] [block_number] [to_block]
 */

import { ApiPromise, WsProvider } from "@polkadot/api";
import { resolveChain } from "./utils/networks.ts";
import { getBlockRange } from "./db/queries.ts";
import { encodeBlockForImport } from "./utils/scale.ts";
import { closeDb, configureSchema, withDbRetry } from "./db/client.ts";

async function main() {
  // Extract --chain flag
  const rawArgs = process.argv.slice(2);
  let chainArg: string | undefined;
  const positional: string[] = [];

  for (let i = 0; i < rawArgs.length; i++) {
    if (rawArgs[i] === "--chain" && i + 1 < rawArgs.length) {
      chainArg = rawArgs[++i];
    } else {
      positional.push(rawArgs[i]!);
    }
  }

  const chain = resolveChain(chainArg);
  await configureSchema(chain.chainId);

  const blockNum = parseInt(positional[0] || "1", 10);
  const toBlock = parseInt(positional[1] || String(blockNum), 10);

  console.log(`Chain: ${chain.displayName} (${chain.chainId})`);
  console.log(`Connecting to: ${chain.rpcEndpoints[0]}`);
  const provider = new WsProvider(chain.rpcEndpoints);
  const api = await ApiPromise.create({ provider });

  let mismatches = 0;
  let checked = 0;

  for (let num = blockNum; num <= toBlock; num++) {
    // 1. Get the authoritative SignedBlock SCALE encoding from RPC
    const blockHash = await api.rpc.chain.getBlockHash(num);
    const signedBlock = await api.rpc.chain.getBlock(blockHash);
    const originalHex = signedBlock.toHex();
    const originalBytes = Buffer.from(
      originalHex.startsWith("0x") ? originalHex.slice(2) : originalHex,
      "hex",
    );

    // 2. Get the same block from our PostgreSQL and reconstruct
    const rows = await withDbRetry(() => getBlockRange(num, num));
    if (rows.length === 0) {
      console.log(`  Block #${num}: NOT IN DB (skipped)`);
      continue;
    }

    const row = rows[0]!;
    const reconstructed = encodeBlockForImport(row);
    // Strip the 4-byte LE length prefix (import-blocks format wrapper)
    const reconstructedBlock = reconstructed.subarray(4);

    // 3. Compare
    checked++;
    if (Buffer.compare(originalBytes, reconstructedBlock) === 0) {
      console.log(`  Block #${num}: OK (${originalBytes.length} bytes)`);
    } else {
      mismatches++;
      console.log(`  Block #${num}: MISMATCH!`);
      console.log(`    Original:      ${originalBytes.length} bytes`);
      console.log(`    Reconstructed: ${reconstructedBlock.length} bytes`);

      // Find first divergence point
      const minLen = Math.min(originalBytes.length, reconstructedBlock.length);
      for (let i = 0; i < minLen; i++) {
        if (originalBytes[i] !== reconstructedBlock[i]) {
          console.log(`    First diff at byte ${i}:`);
          console.log(
            `      Original:      0x${originalBytes.subarray(i, Math.min(i + 32, minLen)).toString("hex")}`,
          );
          console.log(
            `      Reconstructed: 0x${reconstructedBlock.subarray(i, Math.min(i + 32, minLen)).toString("hex")}`,
          );
          break;
        }
      }
      if (originalBytes.length !== reconstructedBlock.length) {
        console.log(
          `    Length diff: original=${originalBytes.length} vs reconstructed=${reconstructedBlock.length}`,
        );
      }
    }
  }

  console.log("");
  console.log(
    `Result: ${checked} blocks checked, ${mismatches} mismatches`,
  );

  if (mismatches > 0) {
    console.log("FAIL: SCALE reconstruction does not match original encoding.");
    console.log(
      "The .bin export is NOT safe for import-blocks. scale.ts needs fixing.",
    );
  } else {
    console.log("PASS: All reconstructed blocks match original RPC encoding.");
  }

  // Ensure closeDb() always runs even if api.disconnect() throws.
  try {
    await api.disconnect();
  } finally {
    await closeDb();
  }
  process.exit(mismatches > 0 ? 1 : 0);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
