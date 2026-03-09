/**
 * Network presets and resolution helper.
 *
 * Resolution order:
 *   1. RPC_ENDPOINT env var — fully custom endpoint
 *   2. NETWORK env var ("mainnet" | "testnet") — select a preset
 *   3. Default: mainnet
 */

export interface NetworkConfig {
  name: string;
  rpcEndpoint: string;
  runtimeName: string;
}

export const NETWORKS: Record<string, NetworkConfig> = {
  mainnet: {
    name: "THX Network Mainnet",
    rpcEndpoint: "wss://node.mainnet.thxnet.org/archive-001/ws",
    runtimeName: "thxnet",
  },
  testnet: {
    name: "THX Network Testnet",
    rpcEndpoint: "wss://node.testnet.thxnet.org/archive-001/ws",
    runtimeName: "thxnet-testnet",
  },
};

/**
 * Resolve the network configuration from environment variables.
 *
 * - If `RPC_ENDPOINT` is set, it overrides the preset endpoint.
 * - If `NETWORK` is set to a known key (e.g. "mainnet", "testnet"), use that preset.
 * - Otherwise default to mainnet.
 */
export function resolveNetwork(): NetworkConfig {
  const networkKey = process.env.NETWORK || "mainnet";
  const preset = NETWORKS[networkKey];

  if (!preset) {
    const validKeys = Object.keys(NETWORKS).join(", ");
    console.error(
      `ERROR: Unknown NETWORK="${networkKey}". Valid values: ${validKeys}`,
    );
    process.exit(1);
  }

  const rpcEndpoint = process.env.RPC_ENDPOINT || preset.rpcEndpoint;

  return {
    ...preset,
    rpcEndpoint,
  };
}
