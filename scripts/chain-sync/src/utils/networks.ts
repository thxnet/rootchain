/**
 * Network presets and resolution helper.
 *
 * Resolution order:
 *   1. --chain CLI flag or CHAIN env var
 *   2. Format: "rootchain:mainnet" | "leafchain:1000:mainnet" etc.
 *   3. Default: rootchain:mainnet
 */

// --- Chain-aware types ---

export interface ChainConfig {
  chainId: string; // schema name: "rootchain_mainnet", "leafchain_1000_mainnet"
  chainType: "rootchain" | "leafchain";
  paraId?: number;
  network: "mainnet" | "testnet";
  displayName: string;
  runtimeName: string; // --chain flag for import-blocks
  binaryName: string; // "polkadot" or "thxnet-leafchain"
  rpcEndpoints: string[]; // WebSocket
  httpRpcEndpoints: string[]; // HTTP
}

// --- Chain registry ---

// TODO: Testnet chains (rootchain:testnet, leafchain:*:testnet) are planned
// but not yet available. CLI help mentions testnet support; add entries here
// when testnet archive nodes are provisioned.
export const CHAINS: ChainConfig[] = [
  {
    chainId: "rootchain_mainnet",
    chainType: "rootchain",
    network: "mainnet",
    displayName: "RootChain Mainnet",
    runtimeName: "thxnet",
    binaryName: "polkadot",
    rpcEndpoints: [
      "wss://node.mainnet.thxnet.org/archive-001/ws",
      "wss://node.mainnet.thxnet.org/archive-002/ws",
    ],
    httpRpcEndpoints: [
      "https://node.mainnet.thxnet.org/archive-001/http-rpc",
      "https://node.mainnet.thxnet.org/archive-002/http-rpc",
    ],
  },
  {
    chainId: "leafchain_1000_mainnet",
    chainType: "leafchain",
    paraId: 1000,
    network: "mainnet",
    displayName: "Leafchain THX Mainnet",
    runtimeName: "thx-mainnet",
    binaryName: "thxnet-leafchain",
    rpcEndpoints: [
      "wss://node.thx.mainnet.thxnet.org/archive-001/ws",
      "wss://node.thx.mainnet.thxnet.org/archive-002/ws",
    ],
    httpRpcEndpoints: [
      "https://node.thx.mainnet.thxnet.org/archive-001/http-rpc",
      "https://node.thx.mainnet.thxnet.org/archive-002/http-rpc",
    ],
  },
  {
    chainId: "leafchain_1001_mainnet",
    chainType: "leafchain",
    paraId: 1001,
    network: "mainnet",
    displayName: "Leafchain LMT Mainnet",
    runtimeName: "lmt-mainnet",
    binaryName: "thxnet-leafchain",
    rpcEndpoints: [
      "wss://node.lmt.mainnet.thxnet.org/archive-001/ws",
      "wss://node.lmt.mainnet.thxnet.org/archive-002/ws",
    ],
    httpRpcEndpoints: [
      "https://node.lmt.mainnet.thxnet.org/archive-001/http-rpc",
      "https://node.lmt.mainnet.thxnet.org/archive-002/http-rpc",
    ],
  },
  {
    chainId: "leafchain_1004_mainnet",
    chainType: "leafchain",
    paraId: 1004,
    network: "mainnet",
    displayName: "Leafchain AVATECT Mainnet",
    runtimeName: "avatect-mainnet",
    binaryName: "thxnet-leafchain",
    rpcEndpoints: [
      "wss://node.avatect.mainnet.thxnet.org/archive-001/ws",
      "wss://node.avatect.mainnet.thxnet.org/archive-002/ws",
    ],
    httpRpcEndpoints: [
      "https://node.avatect.mainnet.thxnet.org/archive-001/http-rpc",
      "https://node.avatect.mainnet.thxnet.org/archive-002/http-rpc",
    ],
  },
  {
    chainId: "leafchain_1005_mainnet",
    chainType: "leafchain",
    paraId: 1005,
    network: "mainnet",
    displayName: "Leafchain ECQ Mainnet",
    runtimeName: "ecq-mainnet",
    binaryName: "thxnet-leafchain",
    rpcEndpoints: ["wss://node.ecq.mainnet.thxnet.org/archive-002/ws"],
    httpRpcEndpoints: [
      "https://node.ecq.mainnet.thxnet.org/archive-002/http-rpc",
    ],
  },
];

/**
 * Parse a chain specifier string into a ChainConfig.
 *
 * Formats:
 *   "rootchain:mainnet"         -> rootchain mainnet
 *   "leafchain:1000:mainnet"    -> leafchain paraId 1000 mainnet
 *   "rootchain:testnet"         -> rootchain testnet (not yet in CHAINS, error)
 *
 * Also supports short forms: "rootchain" (defaults to mainnet)
 */
export function parseChainArg(arg: string): ChainConfig {
  const parts = arg.split(":");

  let chainType: string;
  let network: string;
  let paraId: number | undefined;

  if (parts.length === 1) {
    // "rootchain" or "leafchain" — default to mainnet
    chainType = parts[0]!;
    network = "mainnet";
  } else if (parts.length === 2) {
    // "rootchain:mainnet" or "rootchain:testnet"
    chainType = parts[0]!;
    network = parts[1]!;
  } else if (parts.length === 3) {
    // "leafchain:1000:mainnet"
    chainType = parts[0]!;
    paraId = parseInt(parts[1]!, 10);
    network = parts[2]!;
    if (isNaN(paraId)) {
      console.error(
        `ERROR: Invalid paraId "${parts[1]}" in chain specifier "${arg}"`,
      );
      process.exit(1);
    }
  } else {
    console.error(`ERROR: Invalid chain specifier format: "${arg}"`);
    console.error(
      '  Expected: "rootchain:mainnet", "leafchain:1000:mainnet", etc.',
    );
    process.exit(1);
  }

  const chain = CHAINS.find((c) => {
    if (c.chainType !== chainType) return false;
    if (c.network !== network) return false;
    if (chainType === "leafchain" && c.paraId !== paraId) return false;
    return true;
  });

  if (!chain) {
    const available = CHAINS.map((c) =>
      c.chainType === "leafchain"
        ? `${c.chainType}:${c.paraId}:${c.network}`
        : `${c.chainType}:${c.network}`,
    ).join(", ");
    console.error(`ERROR: Unknown chain "${arg}". Available: ${available}`);
    process.exit(1);
  }

  // Apply env overrides
  const result = { ...chain };
  if (process.env.RPC_ENDPOINT) {
    result.rpcEndpoints = [process.env.RPC_ENDPOINT];
  }
  if (process.env.HTTP_RPC_ENDPOINT) {
    result.httpRpcEndpoints = [process.env.HTTP_RPC_ENDPOINT];
  }

  return result;
}

/**
 * Resolve chain from CHAIN env var or default to rootchain:mainnet.
 */
export function resolveChain(chainArg?: string): ChainConfig {
  const arg = chainArg || process.env.CHAIN || "rootchain:mainnet";
  return parseChainArg(arg);
}
