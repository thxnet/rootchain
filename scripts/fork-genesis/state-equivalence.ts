/**
 * state-equivalence.ts — Compare local forked-chain state against livenet.
 *
 * CLI:
 *   bun run state-equivalence.ts --local ws://127.0.0.1:9931 --livenet wss://...
 *
 * Exit codes:
 *   0   — all checks pass
 *   10  — BALANCE_MISMATCH_<ss58_addr>
 *   11  — FINALITY_RESCUE_MISMATCH
 *   12  — SPEC_VERSION_MISMATCH
 *   1   — unexpected error / missing args / <3 validators returned
 *
 * Output: /tmp/forknet-state-equiv.log
 */

import { ApiPromise, WsProvider, HttpProvider } from "@polkadot/api";
import { BN } from "@polkadot/util";
import { createWriteStream } from "fs";

// ─── Exit codes ───────────────────────────────────────────────────────────────
const EXIT_OK                    = 0;
const EXIT_BALANCE_MISMATCH      = 10;
const EXIT_FINALITY_RESCUE_MISMATCH = 11;
const EXIT_SPEC_VERSION_MISMATCH = 12;
const EXIT_ERROR                 = 1;

// ─── Log file ─────────────────────────────────────────────────────────────────
const LOG_PATH = "/tmp/forknet-state-equiv.log";
const logStream = createWriteStream(LOG_PATH, { flags: "w" });

function log(msg: string): void {
  const line = `[${new Date().toISOString()}] ${msg}`;
  console.log(line);
  logStream.write(line + "\n");
}

function logErr(msg: string): void {
  const line = `[${new Date().toISOString()}] ERROR ${msg}`;
  console.error(line);
  logStream.write(line + "\n");
}

// ─── CLI parsing ──────────────────────────────────────────────────────────────
function parseArgs(): { localUrl: string; livenetUrl: string } {
  const args = process.argv.slice(2);
  let localUrl: string | undefined;
  let livenetUrl: string | undefined;

  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--local" && args[i + 1]) {
      localUrl = args[++i];
    } else if (args[i] === "--livenet" && args[i + 1]) {
      livenetUrl = args[++i];
    }
  }

  if (!localUrl || !livenetUrl) {
    logErr("Usage: bun run state-equivalence.ts --local <ws://...> --livenet <wss://...>");
    process.exit(EXIT_ERROR);
  }

  return { localUrl, livenetUrl };
}

// ─── Connection helpers ───────────────────────────────────────────────────────
//
// Local validators expose HTTP-only JSON-RPC (WS upgrade returns 403).
// Livenet uses standard WS. Detect by URL scheme and pick the right provider.
async function connect(url: string, label: string): Promise<ApiPromise> {
  log(`Connecting to ${label} at ${url}`);
  const isHttp = url.startsWith("http://") || url.startsWith("https://");
  const provider = isHttp ? new HttpProvider(url) : new WsProvider(url);
  const api = await ApiPromise.create({ provider });
  const chain = await api.rpc.system.chain();
  log(`Connected to ${label}: chain=${chain.toString()}`);
  return api;
}

async function disconnect(api: ApiPromise, label: string): Promise<void> {
  try {
    await api.disconnect();
    log(`Disconnected from ${label}`);
  } catch {
    // best-effort
  }
}

// ─── Step A: derive first 3 stash addresses from livenet validators ───────────
async function getLivenetValidatorAddresses(
  livenetApi: ApiPromise,
): Promise<string[]> {
  log("Step A: querying livenet session.validators()");
  const validators = await livenetApi.query.session.validators();
  const addrs = (validators as unknown as { toHuman(): string[] }).toHuman();

  if (!Array.isArray(addrs) || addrs.length < 3) {
    logErr(
      `session.validators() returned ${Array.isArray(addrs) ? addrs.length : "non-array"} — need >= 3. Aborting.`,
    );
    process.exit(EXIT_ERROR);
  }

  const first3 = addrs.slice(0, 3);
  log(`Step A: validator addresses (first 3):`);
  for (let i = 0; i < first3.length; i++) {
    const addr = first3[i];
    if (typeof addr !== "string") {
      throw new Error(
        `validator address at index ${i} is not a string: ${typeof addr}`,
      );
    }
    log(`  ${addr}`);
  }
  return first3;
}

// ─── Step B: compare free balances for each address ──────────────────────────
async function checkBalances(
  localApi: ApiPromise,
  livenetApi: ApiPromise,
  addresses: string[],
): Promise<{ exitCode: number; failAddr: string | null }> {
  log("Step B: comparing free balances for each validator address");

  for (const addr of addresses) {
    const [localAcct, livenetAcct] = await Promise.all([
      localApi.query.system.account(addr),
      livenetApi.query.system.account(addr),
    ]);

    // AccountInfo.data.free is a Balance (u128)
    const localFree: BN = (localAcct as unknown as { data: { free: { toBn(): BN } } }).data.free.toBn();
    const livenetFree: BN = (livenetAcct as unknown as { data: { free: { toBn(): BN } } }).data.free.toBn();

    log(
      `  ${addr}: local=${localFree.toString()} livenet=${livenetFree.toString()}`,
    );

    if (!localFree.eq(livenetFree)) {
      logErr(
        `BALANCE_MISMATCH for ${addr}: local=${localFree.toString()} != livenet=${livenetFree.toString()}`,
      );
      return { exitCode: EXIT_BALANCE_MISMATCH, failAddr: addr };
    }
  }

  log("Step B: PASS — all balances match");
  return { exitCode: EXIT_OK, failAddr: null };
}

// ─── Step C: compare finalityRescue.lastRescueBlock() ────────────────────────
async function checkFinalityRescue(
  localApi: ApiPromise,
  livenetApi: ApiPromise,
): Promise<number> {
  log("Step C: comparing finalityRescue.lastRescueBlock()");

  // This pallet may not exist — handle gracefully
  const hasLocal = localApi.query.finalityRescue !== undefined;
  const hasLivenet = livenetApi.query.finalityRescue !== undefined;

  if (!hasLocal && !hasLivenet) {
    log("Step C: PASS — pallet absent on both sides (None == None)");
    return EXIT_OK;
  }

  if (hasLocal !== hasLivenet) {
    logErr(
      `FINALITY_RESCUE_MISMATCH: pallet present on ${hasLocal ? "local" : "livenet"} only`,
    );
    return EXIT_FINALITY_RESCUE_MISMATCH;
  }

  const [localRaw, livenetRaw] = await Promise.all([
    localApi.query.finalityRescue.lastRescueBlock(),
    livenetApi.query.finalityRescue.lastRescueBlock(),
  ]);

  // Option<BlockNumber> — compare as human-readable strings (None / Some(n))
  const localStr = localRaw.toString();
  const livenetStr = livenetRaw.toString();

  log(`  local lastRescueBlock:   ${localStr}`);
  log(`  livenet lastRescueBlock: ${livenetStr}`);

  if (localStr !== livenetStr) {
    logErr(
      `FINALITY_RESCUE_MISMATCH: local=${localStr} != livenet=${livenetStr}`,
    );
    return EXIT_FINALITY_RESCUE_MISMATCH;
  }

  log("Step C: PASS — lastRescueBlock matches");
  return EXIT_OK;
}

// ─── Step D: compare specVersion ─────────────────────────────────────────────
async function checkSpecVersion(
  localApi: ApiPromise,
  livenetApi: ApiPromise,
): Promise<number> {
  log("Step D: comparing specVersion");

  const [localRv, livenetRv] = await Promise.all([
    localApi.rpc.state.getRuntimeVersion(),
    livenetApi.rpc.state.getRuntimeVersion(),
  ]);

  const localSpec = localRv.specVersion.toNumber();
  const livenetSpec = livenetRv.specVersion.toNumber();

  log(`  local specVersion:   ${localSpec}`);
  log(`  livenet specVersion: ${livenetSpec}`);

  if (localSpec !== livenetSpec) {
    logErr(
      `SPEC_VERSION_MISMATCH: local=${localSpec} != livenet=${livenetSpec}`,
    );
    return EXIT_SPEC_VERSION_MISMATCH;
  }

  log("Step D: PASS — specVersion matches");
  return EXIT_OK;
}

// ─── Main ─────────────────────────────────────────────────────────────────────
async function main(): Promise<number> {
  const { localUrl, livenetUrl } = parseArgs();

  log("=== state-equivalence check START ===");
  log(`local   : ${localUrl}`);
  log(`livenet : ${livenetUrl}`);

  let localApi: ApiPromise | null = null;
  let livenetApi: ApiPromise | null = null;

  try {
    // Connect both in parallel
    [localApi, livenetApi] = await Promise.all([
      connect(localUrl, "local"),
      connect(livenetUrl, "livenet"),
    ]);

    // Step A — get validator addresses from livenet (never hardcoded)
    const addresses = await getLivenetValidatorAddresses(livenetApi);

    // Step B — balance check
    const balResult = await checkBalances(localApi, livenetApi, addresses);
    if (balResult.exitCode !== EXIT_OK) {
      const label = `BALANCE_MISMATCH_${balResult.failAddr ?? "unknown"}`;
      log(`=== state-equivalence check FAILED: ${label} ===`);
      return balResult.exitCode;
    }

    // Step C — finalityRescue
    const rescueCode = await checkFinalityRescue(localApi, livenetApi);
    if (rescueCode !== EXIT_OK) {
      log("=== state-equivalence check FAILED: FINALITY_RESCUE_MISMATCH ===");
      return rescueCode;
    }

    // Step D — specVersion
    const specCode = await checkSpecVersion(localApi, livenetApi);
    if (specCode !== EXIT_OK) {
      log("=== state-equivalence check FAILED: SPEC_VERSION_MISMATCH ===");
      return specCode;
    }

    log("=== state-equivalence check PASSED (all steps) ===");
    return EXIT_OK;
  } catch (err) {
    logErr(`Unexpected error: ${err}`);
    return EXIT_ERROR;
  } finally {
    const disconnects: Promise<void>[] = [];
    if (localApi) disconnects.push(disconnect(localApi, "local"));
    if (livenetApi) disconnects.push(disconnect(livenetApi, "livenet"));
    await Promise.allSettled(disconnects);
    logStream.end();
    // Give stream time to flush before process.exit
    await new Promise<void>((r) => setTimeout(r, 100));
  }
}

main().then((code) => {
  process.exit(code);
});
