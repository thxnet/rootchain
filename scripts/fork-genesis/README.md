# fork-genesis: 3-validator forknet verification

Boot a 3-validator local network from a forked chain spec and verify
liveness + state equivalence against livenet.

## Prerequisites

- **clang-14** build env (set before any Rust build steps):
  ```
  export CC=clang-14
  export CXX=clang++-14
  export LIBCLANG_PATH=/usr/lib/llvm-14/lib
  export CXXFLAGS="-stdlib=libc++"
  ```
- **RocksDB seed** at `/data/forknet-test/rootchain-seed`
  (read-only; fork-genesis copies state from here, never writes)
- **Binary** at `target/release/polkadot` (built from repo root)
- **bun** in PATH (for `state-equivalence.ts`)

## One-liner

```bash
bash scripts/fork-genesis/verify-rootchain.sh
```

Pass `--keep-running` to leave validators alive after the run (debugging).
Pass `--skip-fork-genesis` to reuse an existing `/tmp/forked-thxnet-testnet.json` (≥ 20MB).

## Expected runtime

5–8 minutes per full run (fork-genesis: ~60-120s; block-50 polling: up to 360s).

## Verification criteria

| # | Criterion | Threshold |
|---|-----------|-----------|
| 1 | fork-genesis JSON produced | ≥ 1 MB |
| 2 | `Imported #1` seen in any validator log | within 30s of all-up |
| 3 | First `finalized #N` seen in any validator log | within 60s of all-up |
| 4 | Finalized block ≥ 50 | within 360s |
| 5 | No critical log patterns | 0 hits |
| 6 | Free balance (local vs livenet) | exact match for first 3 validators |
| 7 | `finalityRescue` balance (local vs livenet) | exact match |
| 8 | `specVersion` (local vs livenet) | exact match |
| 9 | state-equivalence.ts exit code | 0 |
| 10 | No orphan validator processes after exit | all PIDs terminated |

## Exit class table

| Exit class | Code | Meaning |
|------------|------|---------|
| Shell-level failures | 1 | Any `die` call exits with code **1**; the LABEL (see below) is emitted to stderr for diagnosis |
| TS exit 10 | 10 | livenet WS connection failed |
| TS exit 11 | 11 | local HTTP connection failed |
| TS exit 12 | 12 | state mismatch (balance / rescue / specVersion) |

Shell diagnostic LABELs (emitted to stderr, NOT exit codes): `FORK_JSON_FAIL`, `ALICE_PEER_ID_MISSING`, `FIRST_IMPORT_TIMEOUT`, `FIRST_FINALIZE_TIMEOUT`, `BLOCK_50_TIMEOUT`, `CRITICAL_LOG_HIT`, `STATE_EQUIV_FAIL`

## Interpreting exit classes

- `FORK_JSON_FAIL` — check `/tmp/forknet-fork-genesis.log`; likely DB path wrong or binary missing
- `ALICE_PEER_ID_MISSING` — Alice crashed at init; read `/tmp/forknet-test-alice.log` top-to-bottom
- `FIRST_IMPORT_TIMEOUT` — peer discovery failed; check bootnode multiaddr and port 40331
- `FIRST_FINALIZE_TIMEOUT` — GRANDPA not converging; verify all 3 validators connected to each other
- `BLOCK_50_TIMEOUT` — authoring stalled; check `CRITICAL_LOG_HIT` patterns in logs
- `CRITICAL_LOG_HIT` — runtime panic or essential-task failure; full log inspection required
- `STATE_EQUIV_FAIL` — fork state diverged from livenet; check `/tmp/forknet-state-equiv.log`
- TS exit 10 — livenet RPC unreachable; check internet / WSS endpoint
- TS exit 11 — local node not accepting HTTP RPC on port 9931; Alice may not be up
- TS exit 12 — balance / rescue amount / specVersion mismatch between local and livenet

## Logs to read

| Log file | Content |
|----------|---------|
| `/tmp/forknet-test-alice.log` | Alice validator (bootnode) output |
| `/tmp/forknet-test-bob.log` | Bob validator output |
| `/tmp/forknet-test-charlie.log` | Charlie validator output |
| `/tmp/forknet-state-equiv.log` | state-equivalence.ts full output |
| `/tmp/forknet-e2e-run.log` | verify-rootchain.sh stdout (if redirected) |
| `/tmp/forknet-fork-genesis.log` | fork-genesis subcommand stderr |

---

## Cross-chain verification (relay + parachain)

`verify-cross-chain.sh` boots a 3-validator relay + 3-collator para chain
(sand_testnet, paraId=1003) from forked specs and verifies cross-chain liveness
end-to-end. It calls `fork-genesis` itself to regenerate the relay spec at start.

### Prerequisites

By default the script expects:

- Para spec at `/tmp/w6-t3-verify.json`
  (produced by leafchain `fork-genesis` — W6)
- Both binaries: `target/release/polkadot` and
  `../leafchains/target/release/thxnet-leafchain`
- Read-only seed DB at `/data/forknet-test/rootchain-seed/`

For CI / GitHub Actions these paths are now overrideable via CLI flags or
`VERIFY_CROSS_CHAIN_*` environment variables, so the runner does **not** need
those exact hardcoded paths as long as equivalent inputs are provided.

The workflow wrapper in `.github/workflows/fork-genesis-cross-chain.yaml`
will also materialize `thxnet-leafchain` into the workspace automatically if no
local runner path exists, by pulling a published OCI image
(`ghcr.io/thxnet/leafchain:feature-fork-genesis` by default) and copying
`/usr/local/bin/thxnet-leafchain` out of it. Operators can override that image
per dispatch via the `leafchain_image` input.

### Run

```bash
bash scripts/fork-genesis/verify-cross-chain.sh
```

The script is now **GitHub Actions ready**:

- all formerly hardcoded binary/spec/seed paths can be overridden by CLI flags
  or `VERIFY_CROSS_CHAIN_*` environment variables
- node state, pid files, and logs can be relocated with `--run-root=PATH`
- the default run root becomes runner-local when `RUNNER_TEMP` / `GITHUB_RUN_ID`
  are present, so CI does not need `/tmp/xcv-*` collisions or `/root/Works/...`
  assumptions
- a manual workflow wrapper lives at
  `.github/workflows/fork-genesis-cross-chain.yaml`

Example CI-friendly invocation when the runner already has the binary:

```bash
export VERIFY_CROSS_CHAIN_POLKADOT_BIN="$GITHUB_WORKSPACE/target/release/polkadot"
export VERIFY_CROSS_CHAIN_LEAFCHAIN_BIN="/runner-assets/leafchains/thxnet-leafchain"
export VERIFY_CROSS_CHAIN_PARA_JSON="/runner-assets/specs/w6-t3-verify.json"
export VERIFY_CROSS_CHAIN_SEED_DB="/runner-assets/rootchain-seed"
export VERIFY_CROSS_CHAIN_RUN_ROOT="$RUNNER_TEMP/verify-cross-chain-$GITHUB_RUN_ID-$GITHUB_RUN_ATTEMPT"

bash scripts/fork-genesis/verify-cross-chain.sh --burn-in-seconds=300
```

If the runner does **not** have `thxnet-leafchain`, dispatch the workflow without
`leafchain_bin`; it will pull the default `ghcr.io/thxnet/leafchain:feature-fork-genesis`
image (or your overridden `leafchain_image`) and copy the binary into
`$GITHUB_WORKSPACE/target/release/thxnet-leafchain` before running the script.

This:

1. **Regenerates the forked relay spec** via `polkadot fork-genesis
   --register-leafchain=1003:/tmp/w6-t3-verify.json …`. The new flag tells
   fork-genesis to register paraId 1003 in the fresh `GenesisConfig.paras`
   vec; `pallet_paras::build()` then writes `Parachains`, `Heads`,
   `CurrentCodeHash`, `CodeByHash`, and `CodeByHashRefs` deterministically
   from the leafchain spec's `:code` and exported genesis state.
2. **Boots Phase A** — 3 relay validators with the fresh spec.
3. **Inserts collator aura keystores** (Alice/Bob/Charlie sr25519).
4. **Boots Phase B** — 3 cumulus collators against the leafchain spec, with
   embedded relay-light-clients pointed at the relay validators.
5. **Polls 7 acceptance criteria** including para Imported #1, peer counts,
   60s burn-in, and clean exit.

### W8 fix recap (resolved by `--register-leafchain` + `fix_para_scheduler_state`)

Two `construct_runtime!` decl-order hazards used to make cross-chain backing
impossible at fork-genesis boot:

- **Session (decl 9) cascade fires before Paras (decl 56)**: scheduler reads
  `paras::parachains() = []`, writes `ValidatorGroups = []`. Collator-protocol
  then emits "no validators assigned to core" forever. Fixed by overwriting
  `ParaScheduler.{ValidatorGroups, AvailabilityCores, SessionStartBlock}`
  post-merge with the correct shuffle.
- **Configuration (decl 51) builds after Session (decl 9)**: session_info
  cascade reads `<configuration::Pallet>::config()` returning defaults, so
  `ParaSessionInfo.Sessions(0)` captures `n_cores=0`, all approval-voting
  fields zero, empty discovery/assignment keys. Fixed by re-encoding
  `Sessions(0)` post-merge with values pulled from the host configuration we
  control + dev_authority_set keys. (Note: `Sessions` uses Identity hashing,
  not Twox64Concat — the storage key is `prefix ++ SCALE(0u32)`.)

`Dmp`, `Hrmp`, and `Ump` are also dropped at the pallet level so their
livenet MQC heads do not survive into the forked spec and trigger the
cumulus-parachain-system `assert_eq!(dmq_head.head(), expected_dmq_mqc_head)`
panic at the collator's first proposal.
