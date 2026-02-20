/**
 * Force Runtime Upgrade Script
 *
 * Uses sudo.sudoUncheckedWeight(system.setCode(wasm)) to force upgrade
 * the runtime on any supported network.
 *
 * Environment variables:
 *   SUDO_SURI   — (required) sudo account SURI
 *   NETWORK     — "mainnet" | "testnet" (default: "mainnet")
 *   RPC_ENDPOINT — override the preset RPC endpoint
 *   WASM_PATH   — override the WASM file path (default: derived from network)
 *
 * Usage:
 *   export SUDO_SURI="0x..."
 *   NETWORK=mainnet  bun run force-runtime-upgrade.ts
 *   NETWORK=testnet  bun run force-runtime-upgrade.ts
 */

import { ApiPromise, HttpProvider, WsProvider } from "@polkadot/api";
import { Keyring } from "@polkadot/keyring";
import { u8aToHex } from "@polkadot/util";
import { cryptoWaitReady } from "@polkadot/util-crypto";
import { readFileSync } from "fs";
import { resolveNetwork } from "./networks";

const network = resolveNetwork();

const SUDO_SURI = process.env.SUDO_SURI;

// Derive default WASM path from runtime name: "thxnet" -> "./thxnet_runtime.compact.compressed.wasm"
// "thxnet-testnet" -> "./thxnet_testnet_runtime.compact.compressed.wasm"
const defaultWasmPath = `./${network.runtimeName.replace(/-/g, "_")}_runtime.compact.compressed.wasm`;
const WASM_PATH = process.env.WASM_PATH || defaultWasmPath;

async function main() {
  if (!SUDO_SURI) {
    console.error("ERROR: SUDO_SURI environment variable is not set.");
    console.error("  export SUDO_SURI='0x...'  # sudo account SURI");
    process.exit(1);
  }

  console.log(`Network: ${network.name}`);
  await cryptoWaitReady();

  // 1. Load WASM
  console.log(`Loading WASM from: ${WASM_PATH}`);
  const wasmBinary = readFileSync(WASM_PATH);

  // Verify magic bytes:
  //   Raw WASM:              0x00 0x61 0x73 0x6d (\0asm)
  //   Substrate compressed:  0x52 0xbc 0x53 0x76 (sp-maybe-compressed-blob)
  const isRawWasm =
    wasmBinary[0] === 0x00 &&
    wasmBinary[1] === 0x61 &&
    wasmBinary[2] === 0x73 &&
    wasmBinary[3] === 0x6d;
  const isCompressedWasm =
    wasmBinary[0] === 0x52 &&
    wasmBinary[1] === 0xbc &&
    wasmBinary[2] === 0x53 &&
    wasmBinary[3] === 0x76;

  if (!isRawWasm && !isCompressedWasm) {
    console.error(
      "ERROR: File does not appear to be a valid WASM binary (bad magic bytes).",
    );
    console.error(
      `  Got: 0x${wasmBinary[0]!.toString(16).padStart(2, "0")} 0x${wasmBinary[1]!.toString(16).padStart(2, "0")} 0x${wasmBinary[2]!.toString(16).padStart(2, "0")} 0x${wasmBinary[3]!.toString(16).padStart(2, "0")}`,
    );
    process.exit(1);
  }
  console.log(
    `WASM format: ${isCompressedWasm ? "Substrate compressed" : "raw WASM"}`,
  );

  const wasmHex = u8aToHex(wasmBinary);
  console.log(
    `WASM size: ${wasmBinary.length} bytes (${wasmHex.length} hex chars)`,
  );

  // 2. Connect to RPC
  console.log(`Connecting to: ${network.rpcEndpoint}`);
  const provider = network.rpcEndpoint.startsWith("http")
    ? new HttpProvider(network.rpcEndpoint)
    : new WsProvider(network.rpcEndpoint);
  const api = await ApiPromise.create({ provider });

  const currentVersion = api.runtimeVersion;
  const currentSpecVersion = currentVersion.specVersion.toNumber();
  console.log(
    `Connected: spec=${currentVersion.specName.toString()}, version=${currentSpecVersion}`,
  );

  // 3. Create sudo keypair
  const keyring = new Keyring({ type: "sr25519" });
  const sudoKey = keyring.addFromUri(SUDO_SURI);
  console.log(`Sudo account: ${sudoKey.address}`);

  // 4. Verify sudo account matches on-chain sudo key
  const onChainSudo = await api.query.sudo.key();
  const onChainSudoStr = onChainSudo.toString();
  console.log(`On-chain sudo: ${onChainSudoStr}`);

  if (sudoKey.address !== onChainSudoStr) {
    console.error("ERROR: Sudo account mismatch!");
    console.error(`  Our account:   ${sudoKey.address}`);
    console.error(`  On-chain sudo: ${onChainSudoStr}`);
    process.exit(1);
  }
  console.log("Sudo account verified OK.");

  // 5. Build system.setCode call
  const setCodeCall = api.tx.system.setCode(wasmHex);

  // 6. Wrap with sudo.sudoUncheckedWeight — large weight to bypass limits
  const weight = {
    refTime: 1_000_000_000_000,
    proofSize: 1_000_000_000_000,
  };
  const sudoCall = api.tx.sudo.sudoUncheckedWeight(setCodeCall, weight);
  console.log(
    `Extrinsic method: ${sudoCall.method.section}.${sudoCall.method.method}`,
  );

  // 7. Submit transaction
  console.log("Submitting runtime upgrade transaction...");

  const isWs = network.rpcEndpoint.startsWith("ws");

  if (isWs) {
    // WebSocket: use subscription to wait for InBlock
    await new Promise<void>((resolve, reject) => {
      sudoCall
        .signAndSend(sudoKey, { nonce: -1 }, (result) => {
          console.log(`  Status: ${result.status.type}`);

          if (result.status.isInBlock) {
            console.log(
              `  Included in block: ${result.status.asInBlock.toHex()}`,
            );

            // Check for sudo.Sudid event to confirm success
            const sudoEvent = result.events.find(({ event }) =>
              api.events.sudo.Sudid.is(event),
            );
            if (sudoEvent) {
              const dispatchResult = (sudoEvent.event.data as any)[0];
              if (dispatchResult.isOk) {
                console.log("  Sudo dispatch: OK");
              } else {
                console.error(
                  `  Sudo dispatch ERROR: ${dispatchResult.asErr.toString()}`,
                );
              }
            }

            // Check for system.CodeUpdated event
            const codeUpdated = result.events.find(({ event }) =>
              api.events.system.CodeUpdated.is(event),
            );
            if (codeUpdated) {
              console.log("  system.CodeUpdated event detected!");
            }

            resolve();
          } else if (result.status.isDropped || result.status.isInvalid) {
            reject(new Error(`Transaction ${result.status.type}`));
          }
        })
        .catch(reject);
    });
  } else {
    // HTTP: no subscription support, submit and poll for inclusion
    const txHash = await sudoCall.signAndSend(sudoKey, { nonce: -1 });
    console.log(`  Transaction hash: ${txHash.toHex()}`);
    console.log("  Polling for block inclusion (HTTP mode)...");

    const MAX_ATTEMPTS = 10;
    const POLL_INTERVAL_MS = 6_000;
    let found = false;

    for (let attempt = 1; attempt <= MAX_ATTEMPTS; attempt++) {
      await new Promise((r) => setTimeout(r, POLL_INTERVAL_MS));
      const latestHeader = await api.rpc.chain.getHeader();
      const latestHash = await api.rpc.chain.getBlockHash(
        latestHeader.number.toNumber(),
      );
      const latestBlock = await api.rpc.chain.getBlock(latestHash);
      const txInBlock = latestBlock.block.extrinsics.some(
        (ext) => ext.hash.toHex() === txHash.toHex(),
      );

      if (txInBlock) {
        console.log(
          `  Included in block #${latestHeader.number.toNumber()} (attempt ${attempt})`,
        );

        // Check events for this block
        const apiAt = await api.at(latestHash);
        const events = await apiAt.query.system.events();
        const sudoEvent = events.find(({ event }) =>
          api.events.sudo.Sudid.is(event),
        );
        if (sudoEvent) {
          const dispatchResult = (sudoEvent.event.data as any)[0];
          if (dispatchResult.isOk) {
            console.log("  Sudo dispatch: OK");
          } else {
            console.error(
              `  Sudo dispatch ERROR: ${dispatchResult.asErr.toString()}`,
            );
          }
        }
        const codeUpdated = events.find(({ event }) =>
          api.events.system.CodeUpdated.is(event),
        );
        if (codeUpdated) {
          console.log("  system.CodeUpdated event detected!");
        }

        found = true;
        break;
      }

      console.log(
        `  Not yet included (attempt ${attempt}/${MAX_ATTEMPTS}, block #${latestHeader.number.toNumber()})`,
      );
    }

    if (!found) {
      console.warn(
        "  Transaction not found after polling. It may still be pending — check block explorer.",
      );
    }
  }

  // 8. Verify runtime version changed
  console.log("Verifying runtime version...");
  const newVersion = await api.rpc.state.getRuntimeVersion();
  const newSpecVersion = newVersion.specVersion.toNumber();
  console.log(`Runtime version after upgrade: spec=${newSpecVersion}`);

  if (newSpecVersion > currentSpecVersion) {
    console.log(
      `Runtime upgrade SUCCESS! (${currentSpecVersion} -> ${newSpecVersion})`,
    );
  } else if (newSpecVersion === currentSpecVersion) {
    console.log(
      "Runtime version unchanged — check block explorer for tx status.",
    );
    console.log("  The upgrade may need more time or may have failed.");
  } else {
    console.log(
      `WARNING: Runtime version decreased (${currentSpecVersion} -> ${newSpecVersion}). This is unexpected.`,
    );
  }

  await api.disconnect();
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
