#!/usr/bin/env bun

import { resolveChain, type ChainConfig } from "./utils/networks.ts";

const USAGE = `
Chain Sync — RootChain/Leafchain <-> PostgreSQL bidirectional sync tool

Usage:
  bun run src/cli.ts [--chain CHAIN] <command> [options]

Chain specifier (--chain or CHAIN env var):
  rootchain:mainnet              RootChain mainnet (default)
  leafchain:1000:mainnet         Leafchain THX (paraId 1000)
  leafchain:1001:mainnet         Leafchain LMT (paraId 1001)
  leafchain:1004:mainnet         Leafchain AVATECT (paraId 1004)
  leafchain:1005:mainnet         Leafchain ECQ (paraId 1005)

Commands:
  migrate                        Create/verify database schema and tables
  backup  [options]              Sync chain data to PostgreSQL
  restore [options]              Export blocks from PostgreSQL to binary file
  status                         Show sync progress and chain info

Backup options:
  --from N                       Start from block N (default: last synced + 1)
  --to N                         Stop at block N (default: finalized head)
  --batch-size N                 Blocks per batch (default: 100)
  --continuous                   Keep running, sync new finalized blocks
  --verify                       Re-fetch and verify stored blocks, repair mismatches

Restore options:
  --output FILE, -o FILE         Output file path (default: <chainId>-blocks.bin)
  --from N                       Start from block N (default: 0)
  --to N                         Stop at block N (default: last synced)

Status options:
  --chain all                    Show status of all registered chains

Requirements:
  Backup source: archive node (--pruning=archive or --pruning=archive-canonical)
  Restore target: MUST use --pruning=archive-canonical to preserve historical state

Environment:
  CHAIN=rootchain:mainnet        Select chain (alternative to --chain)
  RPC_ENDPOINT=wss://...         Override RPC endpoint
  PG_URL=postgresql://...        PostgreSQL connection string (required)
`;

/**
 * Extract --chain flag from args, return [chainArg | undefined, remainingArgs].
 */
function extractChainArg(args: string[]): [string | undefined, string[]] {
  const remaining: string[] = [];
  let chainArg: string | undefined;

  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--chain" && i + 1 < args.length) {
      chainArg = args[++i];
    } else {
      remaining.push(args[i]!);
    }
  }

  return [chainArg, remaining];
}

async function main() {
  const rawArgs = process.argv.slice(2);
  const [chainArg, args] = extractChainArg(rawArgs);
  const command = args[0];
  const commandArgs = args.slice(1);

  // Special case: "status --chain all" is handled by extracting chainArg = "all"
  const isStatusAll = command === "status" && chainArg === "all";

  switch (command) {
    case "migrate": {
      const chain = resolveChain(chainArg);
      const { runMigrate } = await import("./commands/migrate.ts");
      await runMigrate(chain);
      break;
    }

    case "backup": {
      const chain = resolveChain(chainArg);
      const { runBackup, parseBackupArgs } =
        await import("./commands/backup.ts");
      const opts = parseBackupArgs(commandArgs);
      await runBackup(opts, chain);
      break;
    }

    case "restore": {
      const chain = resolveChain(chainArg);
      const { runRestore, parseRestoreArgs } =
        await import("./commands/restore.ts");
      const opts = parseRestoreArgs(commandArgs);
      await runRestore(opts, chain);
      break;
    }

    case "status": {
      if (isStatusAll) {
        const { runStatusAll } = await import("./commands/status.ts");
        await runStatusAll();
      } else {
        const chain = resolveChain(chainArg);
        const { runStatus } = await import("./commands/status.ts");
        await runStatus(chain);
      }
      break;
    }

    default:
      if (command && command !== "--help" && command !== "-h") {
        console.error(`Unknown command: ${command}`);
        console.log(USAGE);
        process.exit(1);
      } else {
        console.log(USAGE);
      }
      break;
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
