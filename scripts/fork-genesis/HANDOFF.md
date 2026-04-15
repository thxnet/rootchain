# Fork-Genesis Tooling — Handoff

This doc is for someone picking up `polkadot fork-genesis` after the initial
landing. It assumes you've cloned the repo and read nothing else.

If you only have 60 seconds, jump to **[TL;DR](#tldr)**.

---

## TL;DR

You can:

```bash
# 1. One-time setup: build the forked-substrate node binary
cd /root/Works/rootchain
export CC=clang-14 CXX=clang++-14 LIBCLANG_PATH=/usr/lib/llvm-14/lib CXXFLAGS="-include cstdint"
cargo build --release -p polkadot                           # ~12 min cold

# 2. Boot a forked rootchain (3 dev validators) end-to-end
bash scripts/fork-genesis/verify-rootchain.sh               # ~6 min, exits 0 on PASS

# 3. Boot a forked rootchain + sand leafchain cross-chain devnet
bash scripts/fork-genesis/verify-cross-chain.sh             # ~5 min, 7/7 PASS
```

Both scripts call `polkadot fork-genesis` internally, regenerate specs from a
read-only seed DB at `/data/forknet-test/rootchain-seed/`, boot multi-node
networks, and either cleanly exit (PASS) or `die FATAL[CODE]` (failure mode
documented in script header comment).

What you get: a self-contained 6-node devnet (3 validators + 3 collators)
where the relay chain has all livenet account balances + DAO state + custom
pallet state preserved, but is signed by Alice/Bob/Charlie instead of the
real validator set. Cross-chain backing + inclusion + finality propagate
from block #1.

---

## What problem this solves

**You want a local devnet that mirrors livenet state but uses your own
validators.** Common reasons:

- Test runtime upgrades against real account distributions before pushing to
  testnet
- Reproduce a livenet bug under a controlled session set you sign with your
  own keys
- Develop new parachains against a relay chain that has the real
  configuration / channel state, but boots in seconds without waiting for
  validator coordination

Without `fork-genesis` you'd have to either run the real testnet (slow,
shared) or write a custom dev chain spec with synthetic state (loses
livenet realism).

---

## How it works (architecture)

```
[livenet RocksDB seed]
        │
        ▼ (fork-genesis CLI)
sc_service::chain_ops::export_raw_state(client, finalized_hash)
        │
        ▼  raw Storage { top, children_default }
chain_spec_fork::filter_forked_storage(storage)
        │       drops 19 pallets:
        │       - validator: Babe, Grandpa, Session, Staking, Historical,
        │         ImOnline, AuthorityDiscovery, Offences, VoterList,
        │         ElectionProviderMultiPhase, FastUnstake
        │       - parachain runtime: Paras, ParaInclusion, ParaScheduler,
        │         ParaSessionInfo, ParasDisputes, ParaInherent
        │       - messaging: Dmp, Hrmp, Ump
        │       + drops 13 item-level transients
        │
        ▼  filtered Storage
build_paras_to_register(--register-leafchain flags)
        │       reads each leafchain spec :code from JSON
        │       subprocess → leafchain export-genesis-state → 98-byte head
        │       → ParaGenesisArgs { genesis_head, validation_code, Parachain }
        │
        ▼  Vec<(ParaId, ParaGenesisArgs)>
chain_spec_fork::assemble_thxnet_{testnet,mainnet}_fork_genesis(...)
        │       fills in fresh GenesisConfig with:
        │       - dev_authority_set() = Alice/Bob/Charlie (8 keys each)
        │       - paras_to_register routed through pallet_paras::build()
        │       - SudoConfig.key = Alice
        │       - ConfigurationConfig from default_parachains_host_configuration()
        │
        ▼  Runtime GenesisConfig
genesis.build_storage()    [routes through pallet_session/cascade]
        │       Session(decl 9) cascade fires Initializer→Scheduler→SessionInfo
        │       BUT Paras(decl 56) and Configuration(decl 51) haven't built yet,
        │       so cascade reads paras::parachains()=[] and default config →
        │       writes ValidatorGroups=[] and SessionInfo.{n_cores, validator_groups,
        │       discovery_keys, assignment_keys, ...} all empty
        │
        ▼  fresh Storage (with broken scheduler + session_info state)
merge_storage(filtered, fresh)            [fresh-wins on overlap]
        │
        ▼  merged Storage
chain_spec_fork::fix_para_scheduler_state(&mut merged)   [W8 LOAD-BEARING FIX]
        │       reads merged: Session.Validators (count) + Paras.Parachains (count)
        │       computes scheduler shuffle (mirror of scheduler.rs:268-299)
        │       overwrites:
        │         ParaScheduler.ValidatorGroups       = [[0,1,2]] for 3 validators / 1 core
        │         ParaScheduler.AvailabilityCores     = [None]
        │         ParaScheduler.SessionStartBlock     = 0
        │       then calls fix_para_session_info_session_zero (also LOAD-BEARING):
        │         decode ParaSessionInfo.Sessions(0)   [Identity hashing!]
        │         patch validator_groups + n_cores + 6 config fields +
        │              discovery_keys + assignment_keys
        │         re-encode, write
        │
        ▼  merged Storage (now consistent)
[optional: --runtime-wasm overrides :code]
        │
        ▼
ChainSpec::set_storage(merged) + build_spec(raw=true)
        │
        ▼
forked-spec.json    →   bootable!
```

The W8 retro at `.agent-team-waves/wave-8.md` has full forensic detail on
why each step is necessary. The two `fix_para_*` functions exist because
`construct_runtime!` declaration order forces Session's genesis cascade to
run before Paras and Configuration — see [Lessons learned](#lessons-learned).

---

## Quick-start

### Single-chain rootchain devnet (no parachains)

```bash
bash scripts/fork-genesis/verify-rootchain.sh
```

What it does:
1. Cleans any prior run (`/tmp/forknet-test-*`)
2. Calls `polkadot fork-genesis --chain=thxnet-testnet --base-path=/data/forknet-test/rootchain-seed --database=rocksdb --output=/tmp/forked-thxnet-testnet.json`
3. Boots Alice/Bob/Charlie validators on ports 40331-3 (p2p) / 9931-3 (rpc)
4. Polls for finalized #1 within 60s, then 50 finalized blocks within 6 min
5. Runs `state-equivalence.ts` (Bun + @polkadot/api) to compare 3 random
   livenet account balances against the forked chain's RPC
6. Cleans up; exits 0 on PASS

To inspect manually after boot, pass `--keep-running`:
```bash
bash scripts/fork-genesis/verify-rootchain.sh --keep-running
# then in another terminal:
curl -sS http://127.0.0.1:9931 -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"chain_getHeader","params":[],"id":1}' | jq
```

### Cross-chain devnet (relay + sand leafchain)

```bash
bash scripts/fork-genesis/verify-cross-chain.sh
```

What it does:
1. Cleans prior run (`/tmp/xcv-*`)
2. Calls `polkadot fork-genesis --register-leafchain=1003:/tmp/w6-t3-verify.json --output=/tmp/forked-thxnet-testnet-w8.json`
3. Boots 3 relay validators (40331-3 / 9931-3)
4. Inserts Aura keys into 3 collator keystores
5. Boots 3 cumulus collators against `/tmp/w6-t3-verify.json` with embedded
   relay-light-clients on ports 40335/40337/40339
6. Polls for relay finalized #1, then para Imported #1, then 60s burn-in
7. Cleans up; exits 0 on PASS

Same `--keep-running` pattern works.

---

## How to extend

### A. Fork a different paraId (e.g. a non-sand leafchain)

You need a fresh leafchain chain-spec with a `:code` and `genesis.raw.top`.
The leafchain's own `fork-genesis` (in [`/root/Works/leafchains/`](../../../leafchains/))
produces this — see `cli/src/fork_genesis_cmd.rs` over there.

Then either:

```bash
# Direct CLI use:
polkadot fork-genesis \
  --chain=thxnet-testnet \
  --base-path=/data/forknet-test/rootchain-seed \
  --database=rocksdb \
  --register-leafchain=1003:/path/to/sand-spec.json \
  --register-leafchain=1004:/path/to/aether-spec.json \
  --output=/tmp/forked-multi.json
```

Or modify `verify-cross-chain.sh:48-50`:
```bash
PARA_JSON_SAND="/tmp/w6-t3-verify.json"
PARA_JSON_AETHER="/path/to/aether.json"
REGISTER_FLAGS=(
  "--register-leafchain=1003:${PARA_JSON_SAND}"
  "--register-leafchain=1004:${PARA_JSON_AETHER}"
)
```

**Important caveat**: with N parachains and only M=3 validators, scheduler
forms `[[0],[1],[2],[],...]`-style groups — empty groups can't back any
candidate, so backing for some paras stalls during group rotation
(`group_rotation_frequency=20` blocks by default). For multi-para, either:

- Reduce to ≤3 paras for stable backing
- Increase validators by adding more `--alice/--bob/--charlie/--dave/...`
  to `dev_authority_set()` in `node/service/src/chain_spec_fork.rs:265-271`
  (sp_keyring supports up to Ferdie = 6 validators)
- Bump `max_validators_per_core` in
  `node/service/src/chain_spec.rs:182-222::default_parachains_host_configuration`

### B. Fork mainnet instead of testnet

```bash
polkadot fork-genesis --chain=thxnet --base-path=/path/to/mainnet-seed \
  --database=rocksdb --output=/tmp/forked-mainnet.json
```

The `assemble_thxnet_mainnet_fork_genesis` codepath has been kept in sync
with testnet's API but only testnet has been E2E-verified. Mainnet should
work but expect to debug 1-2 differences (e.g. `polkadot::BABE_GENESIS_EPOCH_CONFIG`
vs `thxnet_testnet::BABE_GENESIS_EPOCH_CONFIG`).

### C. Add or change drop list

`node/service/src/chain_spec_fork.rs:105-128`:
```rust
static DROP_PALLETS: &[&[u8]] = &[
    b"AuthorityDiscovery", b"Babe", b"Dmp", ...
];
static DROP_ITEMS: &[(&[u8], &[u8])] = &[
    (b"System", b"BlockHash"), ...
];
```

Add to the appropriate list. The lists must be sorted lexicographically
(enforced by `drop_lists_are_deduplicated_and_sorted` test). After editing,
run:
```bash
cargo test -p polkadot-service --features polkadot-native --lib chain_spec_fork::
```

### D. Add unit tests

All filter tests live in `node/service/src/chain_spec_fork.rs:tests`. Pattern:
```rust
#[test]
fn drop_my_new_pallet_item() {
    let key = keyed(&item_prefix(b"MyPallet", b"MyItem"), &[0u8; 16]);
    let input = storage_with_keys(&[key.clone()]);
    let output = filter_forked_storage(input);
    assert!(!output.top.contains_key(&key), "MyPallet.MyItem must drop");
}
```

### E. Override the runtime WASM at fork time

```bash
polkadot fork-genesis --chain=thxnet-testnet \
  --base-path=/data/forknet-test/rootchain-seed --database=rocksdb \
  --runtime-wasm=/path/to/new-runtime.compact.compressed.wasm \
  --output=/tmp/upgraded.json
```

Useful for testing a runtime upgrade against fork state without going
through the on-chain governance path. The `--runtime-wasm` override is
applied AFTER all merge logic, so it always wins.

### F. Add a new CLI flag

The flag pipeline lives in `cli/src/fork_genesis_cmd.rs:31-79` (struct) and
its `run()` body. Add a `#[arg(long)] pub my_flag: ...` field to
`ForkGenesisCmd`, parse it in `run()`, thread the value through
`select_runtime_and_assemble_fresh` if it affects fresh genesis.

---

## Lessons learned (read before extending)

These are the landmines we hit. Future work should respect them.

### 1. `construct_runtime!` declaration order matters for genesis cascades

In polkadot-v0.9.40, `pallet_session::build_storage()` triggers the
`SessionHandler` cascade (Initializer + ParaSessionInfo via `SessionKeys.
para_validator + para_assignment`). This cascade runs **at the moment Session
builds**, not after all pallets have built.

If your runtime declares Session with a lower index than Paras/Configuration
(in this fork: Session=9, Configuration=51, Paras=56), then at cascade time
`paras::parachains()` returns empty AND `<configuration::Pallet>::config()`
returns defaults. Anything cascade reads from those pallets is wrong.

**The fix here**: post-build storage overrides
(`fix_para_scheduler_state` + `fix_para_session_info_session_zero`) compute
the correct values from the FINAL merged storage and overwrite what the
cascade wrote.

If you upgrade the SDK and the cascade order changes, these post-build
patches may become unnecessary OR may need updating. Always verify by
checking `paraScheduler.validatorGroups` and
`paraSessionInfo.sessions(0).validatorGroups` over RPC after boot.

### 2. Storage hasher type matters when computing keys manually

`pallet_paras::Heads<T>` uses `Twox64Concat` (44-byte key);
`pallet_session_info::Sessions<T>` uses `Identity` (36-byte key, no concat
hash, just `prefix ++ SCALE(key)`).

We hit this bug in W8. The first `fix_para_session_info_session_zero` used
Twox64Concat key derivation and silently wrote to a non-existent key,
leaving real `Sessions(0)` untouched. The fix only became visible after
post-boot RPC inspection.

**Always grep for the storage type definition before computing a key**:
```bash
rg 'pub.*type SomeStorage<' runtime/parachains/src/
```
Look for the second generic param: `Identity` / `Twox64Concat` /
`Twox128` / `Blake2_128Concat`. Each has a distinct key derivation.

### 3. Misidentifying a storage key by hash collision

W7 thought `Paras.Heads` was at hash `281e0bfd…`. That hash is actually
`Paras.ParaLifecycles`. The W7 patch script silently corrupted ParaLifecycles
without ever touching Heads.

**Rule**: if you're computing storage prefixes by `twox_128`, immediately
verify against another path. Either compute via `bun + @polkadot/util-crypto`
AND check at least one storage value's content matches the expected SCALE
schema for that storage item, OR query the storage path via subxt /
polkadot-js with the same key the runtime uses.

### 4. Filter is not enough; cumulus parachain-system asserts MQC heads

W7 only dropped `(Dmp, DownwardMessageQueues)`. W8 found that
`Dmp.DownwardMessageQueueHeads` (the MQC chain head, separate item)
survived from livenet. At the collator's first proposal, cumulus
parachain-system:861 panics:
```
assert_eq!(dmq_head.head(), expected_dmq_mqc_head)
```

**Rule**: when a fork drops queue contents, ALSO drop any MQC head /
state-root storage items in the same pallet. W8 promoted Dmp + Hrmp + Ump
to whole-pallet drops to be safe.

### 5. fork-genesis's `Imported #1` log is NOT proof of cross-chain liveness

The collator imports its own block 1 locally regardless of whether the
relay backed it. To verify REAL cross-chain liveness, query relay state:

```bash
bun -e "
import { ApiPromise, HttpProvider } from '@polkadot/api';
const api = await ApiPromise.create({ provider: new HttpProvider('http://127.0.0.1:9931') });
const heads = await api.query.paras.heads(1003);
console.log('Paras.Heads(1003) =', heads.toHex().slice(0, 50));
// Should be 0x8901<98 bytes of header> at genesis, then advance with each Included event
const events = await api.query.system.events.at(await api.rpc.chain.getBlockHash());
const para = events.toArray().filter(r => r.event.section === 'paraInclusion');
console.log('paraInclusion events:', para.map(r => r.event.method));
"
```

If `paras.heads(1003)` advances and `paraInclusion.CandidateBacked +
CandidateIncluded` events fire each block → real cross-chain liveness. If
not, you have the W7-style false-positive issue.

### 6. The `verify-cross-chain.sh` SCORECARD `Criterion 4 PASS` is also
weak (matches log pattern, not relay state). Always cross-check with the
RPC query above before declaring victory.

---

## Known limitations / future work

- **Multi-para devnets need ≥N validators** for N paras. Currently
  `dev_authority_set()` is hardcoded to Alice/Bob/Charlie. To support
  larger devnets, parametrize the authority set (CLI flag or constant in
  chain_spec_fork.rs).
- **Mainnet path untested in real boot.** Code compiles and tests pass; E2E
  not run because no mainnet seed DB is locally synced.
- **AuthorityDiscovery storage stays empty at genesis** (matches normal
  polkadot config). Discovery_keys for Sessions(0) come from our patch via
  `dev_authority_set()`, but the AuthorityDiscovery pallet itself has no
  state. Validators may have suboptimal initial peer discovery. Backing
  works regardless because peer-to-peer happens over libp2p / relay's
  normal network. **Low priority.**
- **`scheduler.rs` uses BABE-derived random shuffle** for ValidatorGroups.
  Our `fix_para_scheduler_state` uses the deterministic `[[0,1,2]]` order
  (no shuffle), which differs from what the runtime would produce. For
  N=1 core / M=3 validators, this is identical (one group of all).
  For larger N, this approximation may not match runtime behaviour. If you
  add tests verifying scheduler matches runtime expectations, port the
  shuffle from `runtime/parachains/src/shared.rs::initializer_on_new_session`.
- **No XCM cross-chain message tests.** Only para→relay backing is
  verified. XCM transport between paras was out of scope.

---

## Repository layout (where things live)

```
/root/Works/rootchain/                      <- this repo (forked polkadot-sdk)
├── cli/
│   └── src/
│       └── fork_genesis_cmd.rs             <- ForkGenesisCmd struct + run()
│                                              CLI flag definitions, subprocess
│                                              to leafchain export-genesis-state
├── node/service/src/
│   ├── chain_spec.rs                       <- existing chain spec (W2 only
│   │                                          modified visibility)
│   └── chain_spec_fork.rs                  <- LOAD-BEARING:
│                                              - DROP_PALLETS / DROP_ITEMS
│                                              - filter_forked_storage()
│                                              - dev_authority_set()
│                                              - assemble_thxnet_*_fork_genesis()
│                                              - fix_para_scheduler_state()
│                                              - fix_para_session_info_session_zero()
│                                              - 51 unit tests
├── runtime/                                <- FROZEN; never edit
├── primitives/                             <- FROZEN; we read these types
└── scripts/
    └── fork-genesis/
        ├── README.md                       <- usage docs
        ├── HANDOFF.md                      <- this file
        ├── verify-rootchain.sh             <- single-chain E2E
        ├── verify-cross-chain.sh           <- 6-node cross-chain E2E
        ├── state-equivalence.ts            <- livenet vs forked RPC check
        ├── package.json + bun.lock         <- @polkadot/api dep
        └── tsconfig.json

/root/Works/leafchains/                     <- separate repo
├── node/src/
│   ├── chain_spec/fork.rs                  <- mirror of chain_spec_fork.rs
│   │                                          for leafchain (Aura instead of
│   │                                          BABE/Grandpa)
│   └── fork_genesis_cmd.rs                 <- thxnet-leafchain fork-genesis CLI

/data/forknet-test/                         <- READ-ONLY seed DBs
├── rootchain-seed/                         <- 52 GB RocksDB, fast-synced
└── leafchain-sand-seed/                    <- 139 GB ParityDB, archive

/tmp/                                       <- runtime artifacts (not committed)
├── forked-thxnet-testnet.json              <- W4 single-chain output
├── forked-thxnet-testnet-w8.json           <- W8 cross-chain output
├── w6-t3-verify.json                       <- W6 leafchain spec
├── forknet-test-{alice,bob,charlie}/       <- W4 validator base-paths
├── xcv-relay-{alice,bob,charlie}/          <- W7+W8 relay base-paths
└── xcv-sand-{alice,bob,charlie}/           <- W7+W8 collator base-paths

.agent-team-waves/wave-{1..8}.md            <- full retros (read for context)
```

---

## Testing commands

```bash
# Quick: just the new fork-genesis filter + assembler tests
cargo test -p polkadot-service --features polkadot-native --lib chain_spec_fork::

# CLI tests
cargo test -p polkadot-cli --features polkadot-native --lib fork_genesis_cmd::

# Single-chain E2E (~6 min, real binary boot)
bash scripts/fork-genesis/verify-rootchain.sh

# Cross-chain E2E (~5 min)
bash scripts/fork-genesis/verify-cross-chain.sh

# 15-min full burn-in (most stringent)
bash scripts/fork-genesis/verify-cross-chain.sh --burn-in-seconds=900

# Keep nodes running for manual inspection
bash scripts/fork-genesis/verify-cross-chain.sh --keep-running
```

All tests must pass before any change to `chain_spec_fork.rs` or
`fork_genesis_cmd.rs` lands.

---

## Where to ask for help

- W8 retro at `.agent-team-waves/wave-8.md` — full forensic detail of
  what each fix does and why
- Earlier retros `wave-1.md` through `wave-7.md` — historical context;
  W7 conclusions are largely WRONG (W8 corrects them — see W8 retro
  "What Was Tried But Did Not Work" section)
- Memory dirs `/root/.claude/projects/-root-Works-rootchain/memory/`
  and `/root/Phasmatodea/AI_MEMORIES/` — long-term lessons
- Branch `feature/fork-genesis-cli` on `origin` (thxnet/rootchain) —
  the commit that landed all this
