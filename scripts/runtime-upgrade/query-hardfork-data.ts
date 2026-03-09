/**
 * EXHAUSTIVE scan of blocks 14,195,952 ~ 14,215,952 for setId transitions.
 *
 * Queries EVERY block in the range on BOTH archive nodes via HTTP RPC,
 * then verifies against the data burned into grandpa_support.rs.
 *
 * Usage: bun run query-hardfork-data.ts
 */

import { ApiPromise, HttpProvider } from "@polkadot/api";
import type { BlockHash } from "@polkadot/types/interfaces";
import type { u64 } from "@polkadot/types";
import { Keyring } from "@polkadot/keyring";

const ARCHIVE_ENDPOINTS = [
  "https://node.mainnet.thxnet.org/archive-001/http-rpc",
  "https://node.mainnet.thxnet.org/archive-002/http-rpc",
];

const SCAN_START = 14_195_952;
const SCAN_END = 14_215_952;
const BATCH_SIZE = 30;
const BATCH_DELAY_MS = 200;

// Expected values burned into grandpa_support.rs
const EXPECTED_TRANSITIONS = [
  { setId: 988, block: 14_206_564, hash: "0xb24efda871e72649a6512d418e75b5e5e5921307ee04564042f3bd1cdd721d04" },
  { setId: 990, block: 14_206_572, hash: "0xc644bb6f30e56c2a8748b38bd3713c6923b6c535923f20fef339c93e83b17c75" },
  { setId: 991, block: 14_206_589, hash: "0x8c0c9f7854a60d23c34e2f1b3c0084ae19427cda17b916d56070662b33018a52" },
  { setId: 992, block: 14_206_590, hash: "0x2b86dabc7f9f2c0169d25cfeb9e7766a18ab6ed2fbde00022d9b729d85ac96b3" },
  { setId: 991, block: 14_206_591, hash: "0x323b1605b3030e79bae563f64e5c7f5cad9147632a0230764744d6f04e190b9f" },
  { setId: 992, block: 14_206_626, hash: "0x9db27f4ec24dc50ca5c314a76f55384748ee6d0a1af3f719ec07166238a8200c" },
];
const EXPECTED_LAST_FINALIZED = "0x74e947074c278561bfb924df4a173735c827b53a6f0f0ac8416c1ac99eed0150";
const EXPECTED_AUTHORITIES = [
  "5DQjEK2cWN2Qnp5sFdJQAoQ5RLaveyCxYpCbc8kWK2mbkrHi",
  "5CLCUaSjUhmukZEsp9bTgWi6gBDCMEVLXebN79U46q68Qzh1",
  "5FMYd9YVje234kxfCwZ5UmWoEQ6Zjz78GjjN3hQLM7SH3wDi",
  "5CNfCS5SZ6zEu9YtW1HKeyBxWibrwedgd6by4y9W1D2R1NbA",
  "5CKRFQnViKUtpyEmETsG2TxmzbWHDpGt9n9r1NWEVh9CU4RY",
  "5FW1LVeZKtrJB8RE3uWSEVsXSyFEkJA6PF5oEeKAnwi8cUMq",
  "5Dn9oyDjpcm6yp3bNRnsHEDgzxnkRgqvinChpt3WfZScjt48",
  "5FWBTpBSv4vCR4SC5Q5XT4zGvXF3cAT7AHfe7i45yRdUxwAL",
  "5Fv7rAvMJaKGEWJr1DNxhn5AeaiPot2TuHNvsCzAk9LyPDLR",
  "5ECrXnTf7R7W5wF8bv4xJiJYYyQUgnZfGy6uce4t36puANrT",
];

interface SetIdTransition {
  setId: number;
  blockNumber: number;
  blockHash: string;
}

function sleep(ms: number) {
  return new Promise((r) => setTimeout(r, ms));
}

async function connectToArchive(endpoint: string): Promise<ApiPromise> {
  console.log(`Connecting to ${endpoint}...`);
  const provider = new HttpProvider(endpoint);
  const api = await ApiPromise.create({ provider });
  await api.isReady;
  console.log(`Connected.`);
  return api;
}

async function exhaustiveScan(
  api: ApiPromise,
  label: string,
): Promise<SetIdTransition[]> {
  const transitions: SetIdTransition[] = [];
  let prevSetId: number | null = null;
  const totalBlocks = SCAN_END - SCAN_START + 1;
  let scanned = 0;

  console.log(`\n[${label}] Exhaustive scan: blocks ${SCAN_START} ~ ${SCAN_END} (${totalBlocks} blocks)`);

  for (let batchStart = SCAN_START; batchStart <= SCAN_END; batchStart += BATCH_SIZE) {
    const batchEnd = Math.min(batchStart + BATCH_SIZE - 1, SCAN_END);
    const blockNumbers: number[] = [];
    for (let n = batchStart; n <= batchEnd; n++) blockNumbers.push(n);

    // Retry logic for 429 errors
    let retries = 0;
    let hashes: BlockHash[], setIds: u64[];
    while (true) {
      try {
        hashes = await Promise.all(
          blockNumbers.map((n) => api.rpc.chain.getBlockHash(n)),
        );
        setIds = await Promise.all(
          hashes.map((h) => (api.query.grandpa!.currentSetId as unknown as { at(hash: BlockHash): Promise<u64> }).at(h)),
        );
        break;
      } catch (e: unknown) {
        const msg = e instanceof Error ? e.message : String(e);
        if (retries < 5 && (msg.includes("429") || msg.includes("Too Many"))) {
          retries++;
          const backoff = BATCH_DELAY_MS * retries * 2;
          process.stderr.write(`\n  [429] Rate limited at block #${batchStart}, retry ${retries} after ${backoff}ms...`);
          await sleep(backoff);
        } else {
          throw e;
        }
      }
    }

    for (let i = 0; i < blockNumbers.length; i++) {
      const currentSetId = setIds[i]!.toNumber();
      const blockNumber = blockNumbers[i]!;
      const blockHash = hashes[i]!.toHex();

      if (prevSetId !== null && currentSetId !== prevSetId) {
        console.log(
          `\n  setId ${prevSetId} -> ${currentSetId} at block #${blockNumber} (${blockHash})`,
        );
        transitions.push({ setId: currentSetId, blockNumber, blockHash });
      }
      prevSetId = currentSetId;
    }

    scanned += blockNumbers.length;
    const pct = ((scanned / totalBlocks) * 100).toFixed(1);
    process.stdout.write(`\r  [${label}] Progress: ${pct}% (${scanned}/${totalBlocks}, block #${batchEnd})`);

    await sleep(BATCH_DELAY_MS);
  }

  console.log(`\n  [${label}] Scan complete. Found ${transitions.length} transitions.`);
  return transitions;
}

async function getAuthorities(api: ApiPromise, blockNumber: number): Promise<string[]> {
  const hash = await api.rpc.chain.getBlockHash(blockNumber);
  const encoded = await api.rpc.state.call("GrandpaApi_grandpa_authorities", "0x", hash);

  const hex = encoded.toHex().substring(2);
  const firstByte = parseInt(hex.substring(0, 2), 16);
  let offset = 0;
  let count = 0;

  if ((firstByte & 0x03) === 0) { count = firstByte >> 2; offset = 2; }
  else if ((firstByte & 0x03) === 1) {
    const b0 = parseInt(hex.substring(0, 2), 16);
    const b1 = parseInt(hex.substring(2, 4), 16);
    count = ((b1 << 8) | b0) >> 2; offset = 4;
  } else if ((firstByte & 0x03) === 2) {
    const b0 = parseInt(hex.substring(0, 2), 16);
    const b1 = parseInt(hex.substring(2, 4), 16);
    const b2 = parseInt(hex.substring(4, 6), 16);
    const b3 = parseInt(hex.substring(6, 8), 16);
    count = ((b3 << 24) | (b2 << 16) | (b1 << 8) | b0) >> 2; offset = 8;
  }

  const keyring = new Keyring({ type: "ed25519", ss58Format: 42 });
  const addrs: string[] = [];
  for (let i = 0; i < count; i++) {
    addrs.push(keyring.encodeAddress("0x" + hex.substring(offset, offset + 64), 42));
    offset += 64 + 16;
  }
  return addrs;
}

async function main() {
  console.log("=".repeat(80));
  console.log("GRANDPA AuthoritySetHardFork — EXHAUSTIVE VERIFICATION");
  console.log(`Scanning EVERY block from #${SCAN_START} to #${SCAN_END} (${SCAN_END - SCAN_START + 1} blocks)`);
  console.log("On BOTH archive-001 and archive-002 via HTTP RPC");
  console.log("=".repeat(80));

  // ── Scan archive-001 ──
  const api1 = await connectToArchive(ARCHIVE_ENDPOINTS[0]!);
  const transitions1 = await exhaustiveScan(api1, "archive-001");
  const lfHash1 = (await api1.rpc.chain.getBlockHash(14_205_952)).toHex();
  console.log(`  [archive-001] Last finalized #14,205,952: ${lfHash1}`);
  const auth1 = await getAuthorities(api1, 14_210_000);
  console.log(`  [archive-001] Authorities at #14,210,000: ${auth1.length} validators`);

  // ── Scan archive-002 ──
  const api2 = await connectToArchive(ARCHIVE_ENDPOINTS[1]!);
  const transitions2 = await exhaustiveScan(api2, "archive-002");
  const lfHash2 = (await api2.rpc.chain.getBlockHash(14_205_952)).toHex();
  console.log(`  [archive-002] Last finalized #14,205,952: ${lfHash2}`);
  const auth2 = await getAuthorities(api2, 14_210_000);
  console.log(`  [archive-002] Authorities at #14,210,000: ${auth2.length} validators`);

  // ── Results ──
  console.log("\n" + "=".repeat(80));
  console.log("RESULTS");
  console.log("=".repeat(80));

  let allOk = true;
  const check = (label: string, ok: boolean) => {
    const mark = ok ? "OK " : "FAIL";
    console.log(`  [${mark}] ${label}`);
    if (!ok) allOk = false;
  };

  // Cross-validate
  check(
    "archive-001 vs archive-002: transitions match",
    JSON.stringify(transitions1) === JSON.stringify(transitions2),
  );
  check(
    "archive-001 vs archive-002: last finalized hash match",
    lfHash1 === lfHash2,
  );
  check(
    "archive-001 vs archive-002: authorities match",
    JSON.stringify(auth1) === JSON.stringify(auth2),
  );

  // Verify transition count
  check(
    `Transition count = ${EXPECTED_TRANSITIONS.length} (got ${transitions1.length})`,
    transitions1.length === EXPECTED_TRANSITIONS.length,
  );

  // Verify each transition
  for (let i = 0; i < EXPECTED_TRANSITIONS.length; i++) {
    const exp = EXPECTED_TRANSITIONS[i]!;
    const got = transitions1[i];
    if (!got) {
      check(`Transition[${i}] missing!`, false);
      continue;
    }
    check(
      `Transition[${i}]: setId=${exp.setId} block=#${exp.block}`,
      got.setId === exp.setId && got.blockNumber === exp.block,
    );
    check(
      `Transition[${i}]: hash=${exp.hash.substring(0, 18)}...`,
      got.blockHash === exp.hash,
    );
  }

  // Check no extra transitions
  if (transitions1.length > EXPECTED_TRANSITIONS.length) {
    for (let i = EXPECTED_TRANSITIONS.length; i < transitions1.length; i++) {
      check(
        `UNEXPECTED extra transition: setId=${transitions1[i]!.setId} at #${transitions1[i]!.blockNumber}`,
        false,
      );
    }
  }

  // Verify authorities
  check(`Authorities count = ${EXPECTED_AUTHORITIES.length}`, auth1.length === EXPECTED_AUTHORITIES.length);
  for (let i = 0; i < EXPECTED_AUTHORITIES.length; i++) {
    check(`Authority[${i}]: ${EXPECTED_AUTHORITIES[i]!.substring(0, 16)}...`, auth1[i] === EXPECTED_AUTHORITIES[i]);
  }

  // Verify last finalized
  check(`Last finalized hash matches`, lfHash1 === EXPECTED_LAST_FINALIZED);

  console.log("\n" + "=".repeat(80));

  try { await api1.disconnect(); } catch {}
  try { await api2.disconnect(); } catch {}

  if (allOk) {
    console.log("ALL CHECKS PASSED — data is safe to burn into binary.");
  } else {
    console.log("SOME CHECKS FAILED — DO NOT proceed until resolved!");
    process.exit(1);
  }

  process.exit(0);
}

main().catch((e) => {
  console.error("Fatal error:", e);
  process.exit(1);
});
