import { ApiPromise, WsProvider } from "@polkadot/api";
import type { ChainConfig } from "../utils/networks.ts";
import { createHttpRpcClient, type HttpRpcClient } from "./http-client.ts";
import { encodeCompact } from "../utils/scale.ts";

// Well-known storage keys (xxhash128 of pallet + storage item, constant across all Substrate chains)
const SYSTEM_EVENTS_KEY =
  "0x26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7";
const TIMESTAMP_NOW_KEY =
  "0xf0c365c3cf59d671eb72da0e7a4113c49f1f0515f462cdcf84e0f1d6045dfcbb";

// --- Types ---

export interface FetchedBlock {
  blockNumber: number;
  blockHash: Buffer;
  parentHash: Buffer;
  stateRoot: Buffer;
  extrinsicsRoot: Buffer;
  digest: Buffer;
  extrinsics: Buffer;
  justifications: Buffer | null;
  specVersion: number;
  extrinsicCount: number;
  timestampMs: number | null;
  eventsRaw: Buffer;
  eventCount: number;
  runtimeVersion: {
    specVersion: number;
    specName: string;
    implVersion: number;
  };
}

export interface RpcClient {
  api: ApiPromise;
  httpRpc: HttpRpcClient;
  connectApi(): Promise<ApiPromise>;
  disconnectApi(): Promise<void>;
  /**
   * Fetch a batch of blocks from `fromBlock` to `toBlock` (inclusive).
   * @param signal - Optional callback; if it returns true the inner loop exits
   *   early, preventing wasted HTTP requests after a stop is requested.
   */
  fetchBlockBatch(
    fromBlock: number,
    toBlock: number,
    signal?: () => boolean,
  ): Promise<FetchedBlock[]>;
  getFinalizedHead(): Promise<{ number: number; hash: string }>;
  getMetadataAtBlock(blockHash: string): Promise<Buffer>;
}

function hexToBuffer(hex: string): Buffer {
  return Buffer.from(hex.startsWith("0x") ? hex.slice(2) : hex, "hex");
}

// --- HTTP-based block fetching (high throughput) ---

interface RpcBlockResult {
  block: {
    header: {
      parentHash: string;
      number: string;
      stateRoot: string;
      extrinsicsRoot: string;
      digest: { logs: string[] };
    };
    extrinsics: string[];
  };
  justifications: Array<[string | number[], string | number[]]> | null;
}

interface RpcRuntimeVersion {
  specName: string;
  specVersion: number;
  implVersion: number;
}

function decodeTimestamp(hex: string | null): number | null {
  if (!hex) return null;
  const buf = hexToBuffer(hex);
  if (buf.length < 8) return null;
  const lo = buf.readUInt32LE(0);
  const hi = buf.readUInt32LE(4);
  return hi * 0x100000000 + lo;
}

function countEvents(hex: string | null): number {
  if (!hex) return 0;
  const buf = hexToBuffer(hex);
  if (buf.length === 0) return 0;

  const first = buf[0]!;
  const mode = first & 0x03;
  if (mode === 0) return first >> 2;
  if (mode === 1) {
    if (buf.length < 2) return 0;
    return (buf.readUInt16LE(0) >> 2);
  }
  if (mode === 2) {
    if (buf.length < 4) return 0;
    return (buf.readUInt32LE(0) >> 2);
  }
  // mode === 3: big-integer compact encoding
  // The first byte encodes the number of additional bytes: (first >> 2) + 4
  const extraBytes = (first >> 2) + 4;
  if (buf.length < 1 + extraBytes) return 0;
  let count = 0;
  for (let i = 0; i < extraBytes; i++) {
    count += buf[1 + i]! * (256 ** i);
  }
  return count;
}

function encodeDigestFromLogs(logs: string[]): Buffer {
  const items = logs.map((log) => hexToBuffer(log));
  const parts: Buffer[] = [encodeCompact(items.length)];
  for (const item of items) {
    parts.push(item);
  }
  return Buffer.concat(parts);
}

function encodeExtrinsicsVec(hexExtrinsics: string[]): Buffer {
  const parts: Buffer[] = [encodeCompact(hexExtrinsics.length)];
  for (const hex of hexExtrinsics) {
    const bytes = hexToBuffer(hex);
    parts.push(encodeCompact(bytes.length));
    parts.push(bytes);
  }
  return Buffer.concat(parts);
}

/**
 * Convert a value that may be a hex string or a number[] (raw bytes) to Buffer.
 */
function toBuffer(val: string | number[] | Uint8Array): Buffer {
  if (typeof val === "string") return hexToBuffer(val);
  return Buffer.from(val);
}

function encodeJustifications(
  raw: Array<[string | number[], string | number[]]> | null,
): Buffer | null {
  if (!raw || raw.length === 0) return null;
  const parts: Buffer[] = [encodeCompact(raw.length)];
  for (const [engineId, data] of raw) {
    // ConsensusEngineId: 4 bytes — RPC may return hex string or byte array
    parts.push(toBuffer(engineId));
    // Justification data: Vec<u8>
    const dataBytes = toBuffer(data);
    parts.push(encodeCompact(dataBytes.length));
    parts.push(dataBytes);
  }
  return Buffer.concat(parts);
}

// FETCH_CONCURRENCY controls how many block fetches run in parallel inside
// fetchBlockBatch. Each block fetch issues 5 logical RPC calls (chain_getBlockHash,
// chain_getBlock, state_getRuntimeVersion, state_getStorage ×2) via 2 HTTP
// requests (1 single + 1 batch of 4), so with FETCH_CONCURRENCY = 100 the
// effective peak is up to 200 concurrent HTTP requests. Ensure the RPC
// endpoint pool and OS file-descriptor limits can handle this load before
// increasing further.
const FETCH_CONCURRENCY = 100;

/**
 * Create an instance-based RPC client for a specific chain.
 */
export function createRpcClient(chain: ChainConfig): RpcClient {
  let _apiPromise: Promise<ApiPromise> | null = null;
  const httpRpc = createHttpRpcClient({ endpoints: chain.httpRpcEndpoints });

  async function fetchSingleBlockHttp(blockNum: number): Promise<FetchedBlock> {
    const MAX_RETRIES = 5;

    for (let attempt = 1; attempt <= MAX_RETRIES; attempt++) {
      try {
        const hashHex = await httpRpc.rpcCall<string>("chain_getBlockHash", [blockNum]);

        const [blockResult, runtimeVersion, eventsHex, timestampHex] =
          await httpRpc.rpcBatch<RpcBlockResult | RpcRuntimeVersion | string | null>([
            { method: "chain_getBlock", params: [hashHex] },
            { method: "state_getRuntimeVersion", params: [hashHex] },
            { method: "state_getStorage", params: [SYSTEM_EVENTS_KEY, hashHex] },
            { method: "state_getStorage", params: [TIMESTAMP_NOW_KEY, hashHex] },
          ]) as [RpcBlockResult, RpcRuntimeVersion, string | null, string | null];

        const header = blockResult.block.header;

        return {
          blockNumber: blockNum,
          blockHash: hexToBuffer(hashHex),
          parentHash: hexToBuffer(header.parentHash),
          stateRoot: hexToBuffer(header.stateRoot),
          extrinsicsRoot: hexToBuffer(header.extrinsicsRoot),
          digest: encodeDigestFromLogs(header.digest.logs),
          extrinsics: encodeExtrinsicsVec(blockResult.block.extrinsics),
          justifications: encodeJustifications(blockResult.justifications),
          specVersion: runtimeVersion.specVersion,
          extrinsicCount: blockResult.block.extrinsics.length,
          timestampMs: decodeTimestamp(timestampHex),
          eventsRaw: eventsHex ? hexToBuffer(eventsHex) : Buffer.alloc(0),
          eventCount: countEvents(eventsHex),
          runtimeVersion: {
            specVersion: runtimeVersion.specVersion,
            specName: runtimeVersion.specName,
            implVersion: runtimeVersion.implVersion,
          },
        };
      } catch (err) {
        if (attempt === MAX_RETRIES) {
          throw new Error(
            `Failed to fetch block #${blockNum} after ${MAX_RETRIES} attempts: ${err}`,
          );
        }
        await new Promise((r) =>
          setTimeout(r, 500 * attempt + Math.random() * 500),
        );
      }
    }

    throw new Error(`Failed to fetch block #${blockNum}`);
  }

  const client: RpcClient = {
    get api(): ApiPromise {
      throw new Error("Use connectApi() to get the api instance");
    },

    httpRpc,

    async connectApi(): Promise<ApiPromise> {
      if (_apiPromise) {
        const api = await _apiPromise;
        if (api.isConnected) return api;
        try { await api.disconnect(); } catch {}
        _apiPromise = null;
      }

      _apiPromise = (async () => {
        console.log(`Chain: ${chain.displayName}`);
        console.log(`WS endpoints: ${chain.rpcEndpoints.join(", ")}`);
        console.log(`HTTP endpoints: ${chain.httpRpcEndpoints.join(", ")}`);

        const provider = new WsProvider(chain.rpcEndpoints);
        const api = await ApiPromise.create({ provider });

        console.log(`Connected. Genesis: ${api.genesisHash.toHex()}`);
        return api;
      })();

      return _apiPromise;
    },

    async disconnectApi(): Promise<void> {
      if (_apiPromise) {
        const api = await _apiPromise;
        await api.disconnect();
        _apiPromise = null;
      }
    },

    async fetchBlockBatch(
      fromBlock: number,
      toBlock: number,
      signal?: () => boolean,
    ): Promise<FetchedBlock[]> {
      const results: FetchedBlock[] = [];
      let current = fromBlock;

      while (current <= toBlock) {
        // Respect the stop signal between sub-batches so in-flight requests
        // for already-dispatched blocks still finish, but we stop issuing new
        // ones as soon as the caller requests a stop.
        if (signal?.()) break;

        const batchEnd = Math.min(current + FETCH_CONCURRENCY - 1, toBlock);
        const batch: Promise<FetchedBlock>[] = [];

        for (let blockNum = current; blockNum <= batchEnd; blockNum++) {
          batch.push(fetchSingleBlockHttp(blockNum));
        }

        const fetched = await Promise.all(batch);
        results.push(...fetched);
        current = batchEnd + 1;
      }

      return results;
    },

    async getFinalizedHead(): Promise<{ number: number; hash: string }> {
      const api = await client.connectApi();
      const hash = await api.rpc.chain.getFinalizedHead();
      const header = await api.rpc.chain.getHeader(hash);
      return {
        number: header.number.toNumber(),
        hash: hash.toHex(),
      };
    },

    async getMetadataAtBlock(blockHash: string): Promise<Buffer> {
      const result = await httpRpc.rpcCall<string>("state_getMetadata", [blockHash]);
      return hexToBuffer(result);
    },
  };

  return client;
}

