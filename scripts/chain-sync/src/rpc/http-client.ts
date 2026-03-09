/**
 * Raw JSON-RPC over HTTP with round-robin endpoint selection and auto-retry.
 * Instance-based: each chain gets its own client with independent state.
 */

export interface HttpRpcConfig {
  endpoints: string[];
  maxRetries?: number;
}

export interface HttpRpcClient {
  rpcCall<T = unknown>(method: string, params: unknown[]): Promise<T>;
  rpcBatch<T = unknown>(calls: Array<{ method: string; params: unknown[] }>): Promise<T[]>;
}

export function createHttpRpcClient(config: HttpRpcConfig): HttpRpcClient {
  const endpoints = config.endpoints;
  const maxRetries = config.maxRetries ?? 5;
  let endpointIndex = 0;
  let requestId = 0;

  function nextEndpoint(): string {
    const ep = endpoints[endpointIndex % endpoints.length]!;
    endpointIndex++;
    return ep;
  }

  async function rpcCall<T = unknown>(
    method: string,
    params: unknown[],
  ): Promise<T> {
    for (let attempt = 1; attempt <= maxRetries; attempt++) {
      const endpoint = nextEndpoint();
      try {
        const res = await fetch(endpoint, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            jsonrpc: "2.0",
            id: ++requestId,
            method,
            params,
          }),
        });

        if (!res.ok) {
          throw new Error(`HTTP ${res.status}: ${res.statusText}`);
        }

        const json = (await res.json()) as {
          result?: T;
          error?: { code: number; message: string };
        };

        if (json.error) {
          throw new Error(`RPC error: ${json.error.message} (${json.error.code})`);
        }

        return json.result as T;
      } catch (err) {
        if (attempt === maxRetries) {
          throw new Error(
            `RPC ${method} failed after ${maxRetries} attempts (last endpoint: ${endpoint}): ${err}`,
          );
        }
        await new Promise((r) =>
          setTimeout(r, 500 * attempt + Math.random() * 500),
        );
      }
    }

    throw new Error("Unreachable");
  }

  async function rpcBatch<T = unknown>(
    calls: Array<{ method: string; params: unknown[] }>,
  ): Promise<T[]> {
    if (calls.length === 0) return [];

    for (let attempt = 1; attempt <= maxRetries; attempt++) {
      const endpoint = nextEndpoint();
      try {
        const batch = calls.map((c, i) => ({
          jsonrpc: "2.0",
          id: i + 1,
          method: c.method,
          params: c.params,
        }));

        const res = await fetch(endpoint, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(batch),
        });

        if (!res.ok) {
          throw new Error(`HTTP ${res.status}: ${res.statusText}`);
        }

        const results = (await res.json()) as Array<{
          id: number;
          result?: T;
          error?: { code: number; message: string };
        }>;

        results.sort((a, b) => a.id - b.id);

        if (results.length !== calls.length) {
          throw new Error(`RPC batch: expected ${calls.length} responses, got ${results.length}`);
        }

        for (const r of results) {
          if (r.error) {
            throw new Error(
              `RPC batch error in call #${r.id}: ${r.error.message}`,
            );
          }
        }

        return results.map((r) => r.result as T);
      } catch (err) {
        if (attempt === maxRetries) {
          throw new Error(
            `RPC batch (${calls.length} calls) failed after ${maxRetries} attempts: ${err}`,
          );
        }
        await new Promise((r) =>
          setTimeout(r, 500 * attempt + Math.random() * 500),
        );
      }
    }

    throw new Error("Unreachable");
  }

  return { rpcCall, rpcBatch };
}
