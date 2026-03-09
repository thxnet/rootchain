# Post-Mortem: Mainnet GRANDPA Finality Deadlock

**Date**: 2026-02-20
**Severity**: P0 — Complete mainnet outage
**Duration**: Several hours (from finality stall to full recovery)
**Branch**: `release/runtime-v94000004`
**Affected**: THX Network Mainnet (thxnet), finality stalled from block #14,206,447

---

## Executive Summary

Mainnet validators were rolling-restarted on K8s in batches of 2, with only ~2-minute intervals. This triggered a cascade failure: Kallax PeerInfo sync delays, non-persisted node keys, parachain DB corruption, and a stuck GRANDPA setId. The cumulative effect left all validators effectively offline simultaneously, ultimately causing a permanent GRANDPA finality deadlock at block #14,206,447.

The fix involved three layers: runtime API additions, an on-chain runtime migration, and a node binary hotfix. Critically, **deploying the new node binary to all validator nodes** was the action that actually restored the chain.

---

## Timeline (K8s controller revisions + on-chain block data)

Reconstructed from K8s `controllerrevision` timestamps and on-chain block time analysis (5,000 blocks):

### Phase 1: Original restart — 2 nodes at a time (07:47–07:54 UTC)

K8s controller revision records show validators were restarted in **pairs, at ~2-minute intervals**:

```
07:47:12  aro + bit3x           (Group 1)
07:49:20  mw3w + thxfdn         (Group 2)
07:50:42  thxlab-01 + thxlab-02 (Group 3)
07:52:42  thxlab-03 + thxlab-04 (Group 4)
07:54:15  thxlab-05 + thxlab-06 (Group 5)
```

**Problem**: The 2-minute interval was far too short. A blockchain node needs considerably longer after pod startup to complete peer discovery (Kallax), chain sync, and join GRANDPA voting. By the time Group 3 was killed, Group 1 had likely not yet recovered — the cumulative effect brought all validators offline simultaneously.

### Phase 2: Block production collapse (07:48–08:47 UTC)

On-chain block time data:

| Time (UTC) | Block # | Event |
|------------|---------|-------|
| 07:48 | #14,206,162 | **Incident #1**: 12s block time (Group 1 restart impact begins) |
| 07:49 | #14,206,178 | 18s block time — more validators going offline |
| **08:00** | **#14,206,274** | **Last normal block**. Block production halts completely |
| 08:00–08:38 | — | **38-minute gap with no blocks**: pods showed as Running in K8s, but nodes had not finished syncing / Kallax PeerInfo was not ready / parachain DB state was inconsistent |
| 08:38 | #14,206,275 | First recovery block — **2,280s gap** |
| 08:45 | #14,206,278 | **426s gap** — still unstable |
| 08:46 | #14,206,280–285 | 24s gaps — gradually stabilising |
| 08:47+ | #14,206,286+ | Block production returns to normal 6s |

### Phase 3: Subsequent restart attempts (08:45 + 09:44 + 10:23 UTC)

K8s records show further revision updates (likely debugging / repair attempts):

```
08:45:04–17  ALL 10 validators — new revisions created within the same second (config change)
08:45:15–17  ALL 10 validators — again within the same second (second config adjustment)
09:44:47     thxlab-01 alone
10:23–10:30  ALL 10 validators — ~1-minute intervals, one by one
13:12–13:40  ALL 10 validators — ~3-minute intervals, one by one
```

### Phase 4: Finality dies silently (09:03 UTC)

| Time (UTC) | Block # | Event |
|------------|---------|-------|
| **09:03** | **#14,206,447** | **Finality silently stalls**. Block time remains a perfectly normal 6s — no anomalies whatsoever |
| 09:03 | #14,206,448 | Fork blocks appear → GRANDPA `pending_standard_changes` acquires stale entries |
| 09:03+ | — | Block production continues normally, but finality gap widens steadily; GRANDPA 0/10 votes |

### Phase 5: Hotfix deployment (16:13–16:22 UTC)

New node binary (`release-runtime-v94000004`) deployed at 1 node/min:

```
16:13:19  thxfdn
16:14:10  thxlab-01        (+51s)
16:15:10  thxlab-02        (+60s)
16:16:11  thxlab-03        (+61s)
16:17:08  thxlab-04        (+57s)
16:17:58  thxlab-05        (+50s)
16:18:46  thxlab-06        (+48s)
16:19:32  aro              (+46s) ← FailedCreatePodSandBox (Calico route conflict)
16:20:21  bit3x            (+49s) ← FailedAttachVolume (PVC Multi-Attach error)
16:22:06  mw3w             (+105s)
```

Even at 1 node/min, infrastructure issues surfaced:
- **aro**: Calico network route conflict (stale route from previous pod not cleared)
- **bit3x**: PVC Multi-Attach error (volume still exclusively attached to the old node)

### Phase 6: Recovery (~16:30 UTC)

- All validators running the new binary (with LongestChain hotfix)
- Block production + finality restored to normal
- Finality gap returned to 2 blocks

---

## On-Chain Evidence

Block time statistics from scanning 5,000 blocks (#14,201,447 — #14,206,447):

```
Average: 6.56s | Median: 6.00s | P95: 6.00s | P99: 6.01s | Max: 2,280s
```

Block time anomaly visualisation (each `█` = 1 anomalous block):

```
Normal 6s block production
  │
  ├─ 07:48  #14206162  ████ 12s ──────────── Incident #1: Foreshock (1–2 missed slots)
  ├─ 07:49  #14206178  █████████ 18s
  │
  │  (Normal block production for ~100 blocks)
  │
  ├─ 08:00  #14206274  Last normal block
  │
  │  ██████████████████████████████████████████████████ 38 minutes with zero blocks
  │
  ├─ 08:38  #14206275  ████████████████████████████████████ 2,280s ── Incident #2:
  ├─ 08:45  #14206278  ██████████████████ 426s                        K8s rolling restart
  ├─ 08:46  #14206280  ████ 24s                                       cascade failure
  ├─ 08:46  #14206285  ████ 24s
  │
  │  (Block production returns to normal 6s)
  │  (But finality is already broken — just not visible yet)
  │
  ├─ 09:03  #14206447  Finality silently stalls ─── block time perfectly normal, GRANDPA 0/10 votes
  ├─ 09:03  #14206448  Fork block appears
  │
  │  (Block production continues normally; finality permanently stalled)
  │
  └─ ??? Discovered hours later
```

**The most dangerous aspect**: Block production recovered at 08:47, but the finality deadlock did not occur until 09:03 (#14,206,447), and it presented **absolutely no block time anomalies**. If you only monitor block production, you would believe the chain had recovered.

---

## Root Cause Analysis

### Trigger: Insufficient K8s rolling restart intervals

The restart strategy was **2 nodes at a time, ~2-minute intervals**. This appeared reasonable (10 validators, only 2 disrupted per batch, GRANDPA threshold 7/10), but in reality each node requires far longer than 2 minutes from pod `Running` to **actually participating in consensus**:

```
Pod Running (K8s perspective)  ≠  Validator ready (blockchain perspective)
      │                                  │
      ├── Container started              ├── Chain DB sync
      ├── Readiness probe passes         ├── Kallax PeerInfo sync + peer discovery
      └── ✓ K8s says "Ready"             ├── P2P connections with other validators
                                         ├── BABE slot assignment takes effect
                                         ├── GRANDPA voter joins voting
                                         └── ✓ Actually participating in consensus
                                              (may take 5–15 minutes)
```

**Cascade failure path**:

```
07:47  Kill aro + bit3x (Group 1)
       └── K8s: pods restarting, "Running" in ~30s
       └── Reality: nodes still syncing + discovering peers
07:49  Kill mw3w + thxfdn (Group 2)    ← Group 1 has NOT recovered!
       └── 4 validators effectively offline
07:50  Kill thxlab-01 + 02 (Group 3)   ← Groups 1 & 2 still NOT recovered!
       └── 6 validators effectively offline → below GRANDPA threshold (7/10)
07:52  Kill thxlab-03 + 04 (Group 4)   ← Avalanche
07:54  Kill thxlab-05 + 06 (Group 5)   ← Total collapse
08:00  Last block #14,206,274 → 38 minutes with zero blocks
        │
        ├── Kallax PeerInfo sync delay
        │     └── New pods' IPs not yet broadcast to other validators
        │
        ├── Node keys not persisted
        │     └── P2P identity changed after restart; peer discovery starts from scratch
        │
        ├── Calico network issues (FailedCreatePodSandBox)
        │     └── Stale routes not cleared; new pod network setup fails
        │
        ├── PVC Multi-Attach (FailedAttachVolume)
        │     └── Volume still attached to old node; new pod waits for detach
        │
        └── Parachain DB corruption
              └── chain-selection subsystem missing ~158 block entries
                    └── Fork blocks @ #14,206,448
                          └── GRANDPA AuthoritySet acquires stale entries
                                └── Permanent finality deadlock (silent failure)
```

### Proximate cause: GRANDPA finality deadlock

Fork blocks at #14,206,448 created stale `pending_standard_changes`, causing:

1. `current_limit()` to compute a block limit from stale entries
2. `best_containing(finalized, Some(limit))` to find no valid leaf matching the criteria
3. GRANDPA voters to return `None` → unable to vote → finality permanently stalled
4. Chain-selection DB also missing ~158 block entries (due to parachain DB corruption), compounding the problem

### Why a node binary update was required (not just a runtime upgrade)

| Layer | Fix applied | Why it was insufficient on its own |
|-------|------------|-----------------------------------|
| **Runtime Layer** (on-chain WASM) | ParachainHost v4 API + GRANDPA migration (clear stale state, emit ForcedChange log) | The migration cleared on-chain stale GRANDPA state and emitted a ForcedChange consensus log, but **the node's `SelectRelayChain` still queried the corrupted chain-selection DB**, causing `BestLeafContaining` to continue returning `None` |
| **Node Binary Layer** (off-chain) | `SelectRelayChain::new_with_overseer` → `new_longest_chain` | Bypasses the corrupted chain-selection/approval-voting/dispute-coordinator subsystem; uses longest chain rule to select the best block directly |

**Key insight**: Even after the runtime migration successfully cleared on-chain GRANDPA state, the node binary's `SelectRelayChain` still relied on the off-chain chain-selection DB when choosing which block to vote on. This DB was incomplete following parachain DB corruption, so regardless of on-chain state repairs, finality would not resume as long as the node binary was still using `new_with_overseer` to query the corrupted DB.

---

## What Was Done (Commits)

### 1. `1b0b253fb` — ParachainHost v4 API

The node's dispute-coordinator requires `ParachainHost_disputes` (v3+) and `session_executor_params` (v4) APIs, but the thxnet runtimes only implemented v2. The missing APIs caused chain-selection to return `None`.

**Changed**: `runtime/thxnet/src/lib.rs`, `runtime/thxnet-testnet/src/lib.rs`

### 2. `661fa1b52` — CI Docker Build Fix

`docker/bake-action` v3 is incompatible with buildx >= 0.20.0. Upgraded all Docker GitHub Actions to compatible versions to ensure the new image could be built successfully.

**Changed**: `.github/workflows/integration.yaml`

### 3. `ad015fed5` — GRANDPA Finality Deadlock Migration

On-chain migration:
- Clears all stale GRANDPA state (PendingChange, NextForced, Stalled)
- Schedules a forced authority change via `schedule_change()` (delay=0)
- Emits a `ForcedChange` consensus log during `on_finalize` in the same block
- Increments `CurrentSetId` to align with the GRANDPA client's behaviour

**Changed**: `runtime/thxnet/src/lib.rs` (+122 lines of migration code)

### 4. `381a78c51` — Node Binary Hotfix (THE ACTUAL FIX)

Switches the validator's `SelectRelayChain` from `new_with_overseer` (which depends on the chain-selection subsystem) to `new_longest_chain` (which uses the longest chain rule directly). This bypasses the corrupted off-chain DB state.

**Changed**: `node/service/src/lib.rs` (+5 -5 lines)
**Marked**: `// TODO: Revert to new_with_overseer once finalization catches up.`

---

## Lessons Learned

### Lesson 1: K8s pod "Running" ≠ Blockchain validator "Ready"

**Mistake**: Restarted in batches of 2 nodes with 2-minute intervals. This appeared conservative (only 20% of validators disrupted per batch), but 2 minutes is nowhere near enough for a blockchain node to complete its startup-to-consensus journey.

**Why 2-at-a-time still failed**:
- K8s deems a pod "Running/Ready" in ~30 seconds (container started + readiness probe passes)
- But a blockchain validator actually participating in consensus requires: Kallax PeerInfo sync, P2P peer discovery, chain sync, GRANDPA voter registration — potentially **5–15 minutes**
- Killing the next batch 2 minutes later, before the previous batch had recovered → cumulative effect → all validators offline

**Correct approach**:
- **Custom readiness probe**: Don't just check the HTTP port; verify the node is synced and has sufficient peers
- **Restart only 1 validator at a time**, not 2
- **Gate restart intervals on actual consensus recovery**, not fixed time: wait until the previous validator has genuinely rejoined GRANDPA voting before restarting the next
- Set a `PodDisruptionBudget (PDB)` as a last line of defence

```yaml
# PDB — but this is only a safety net, not a substitute for proper readiness probes
apiVersion: policy/v1
kind: PodDisruptionBudget
metadata:
  name: rootchain-validator-pdb
spec:
  minAvailable: 7  # At least 7 of 10 validators online (GRANDPA threshold)
  selector:
    matchLabels:
      app: rootchain-validator
```

```bash
# Correct manual restart script (conceptual)
for validator in $(kubectl get sts -l role=validator -o name); do
  kubectl rollout restart $validator
  echo "Waiting for $validator to join consensus..."
  # NOT kubectl rollout status (that only waits for K8s Ready)
  # Instead, query RPC to confirm GRANDPA prevotes have recovered to threshold
  while true; do
    prevotes=$(curl -s RPC... | jq '.prevotes.currentWeight')
    [ "$prevotes" -ge 7 ] && break
    sleep 10
  done
  echo "$validator confirmed in consensus. Proceeding to next."
done
```

### Lesson 2: K8s — Node keys and chain data must be persisted

**Mistake**: Node keys were not stored on a PersistentVolume, resulting in new P2P identities being generated after each restart.

**Correct approach**:
- Node key (`/data/chains/thxnet_mainnet/network/secret_ed25519`) **must** reside on a PV
- Chain database **must** reside on a PV with a `storageClassName` supporting high IOPS
- Consider storing the node key in a K8s Secret and mounting it at startup
- Parachain DB (`/data/chains/thxnet_mainnet/parachains/db`) is at high risk of corruption; implement a backup strategy

### Lesson 3: Identify which layer the fix targets before deciding on a deployment strategy

**Mistake**: Performed the runtime upgrade first, expecting it to resolve the issue. The chain remained stalled. Only after deploying the new node binary did the chain recover — creating an unnecessary period of extended downtime.

**Correct understanding**:

```
┌─────────────────────────────────────────────────┐
│  Blockchain Node                                │
│                                                 │
│  ┌─── Node Binary (off-chain) ───────────────┐  │
│  │  • P2P networking                         │  │
│  │  • Block authoring (BABE)                 │  │
│  │  • Finality voting (GRANDPA client)       │  │
│  │  • Chain selection (SelectRelayChain) ◄───┼──┼── Root cause in this incident
│  │  • Subsystems (approval-voting, etc.)     │  │
│  │  • Database (RocksDB)                     │  │
│  └───────────────────────────────────────────┘  │
│                                                 │
│  ┌─── Runtime WASM (on-chain) ───────────────┐  │
│  │  • State transition function              │  │
│  │  • Pallet logic (GRANDPA pallet etc.)     │  │
│  │  • Runtime APIs (ParachainHost etc.)      │  │
│  │  • On-chain migrations                    │  │
│  └───────────────────────────────────────────┘  │
│                                                 │
│  Off-chain fix → must deploy new node binary    │
│  On-chain fix  → runtime upgrade suffices       │
└─────────────────────────────────────────────────┘
```

**Checklist (ask before every hotfix)**:
1. Is the root cause in on-chain logic or off-chain logic?
2. If off-chain → **a new node binary deployment is required**; a runtime upgrade alone is insufficient
3. If on-chain → a runtime upgrade suffices, but verify the migration logic
4. If both → **deploy the node binary first, then perform the runtime upgrade** (or simultaneously)

### Lesson 4: Deployment order — Node binary first, runtime second

**Incorrect order** (what we did):
```
1. Runtime upgrade (spec 94000003 → 94000004)
2. Observe → chain still stalled
3. Only then deploy node binary
4. Chain recovers
```

**Correct order**:
```
1. Deploy new node binary to all validators (rolling update)
2. Confirm validators are running and peer connections are healthy
3. Perform runtime upgrade (if needed)
4. Confirm finality has resumed and block production is normal
```

Why node binary first? Because:
- Node binary updates are **backwards-compatible** (a new binary can run the old runtime)
- Runtime upgrades are **irreversible** (once the on-chain WASM is changed, it's changed)
- If the runtime upgrade has a bug, at least the node binary is sound and the chain can be fixed on-chain
- If the node binary has a bug, a runtime upgrade cannot save you

### Lesson 5: Runtime upgrade tooling must account for HTTP body size limits

**Mistake**: The first attempt to submit the runtime upgrade extrinsic used an HTTP RPC endpoint, resulting in `413 Request Entity Too Large`.

**Correct approach**:
- Runtime WASM is typically 1–2 MB; hex-encoded, the extrinsic reaches 3+ MB
- HTTP RPC endpoints usually have body size limits (at the Nginx/reverse proxy layer)
- **Always use a WebSocket endpoint to submit large extrinsics** (runtime upgrades, large batch calls)
- If HTTP is unavoidable, ensure the reverse proxy's `client_max_body_size` is sufficiently large

### Lesson 6: Finality failures are silent — normal block production does not mean the chain is healthy

**This is the most dangerous lesson from this incident.**

On-chain data proves it:

```
08:47 UTC  Block production returns to normal 6s  ← Appears fine
09:03 UTC  Finality silently stalls                ← Actually broken
???        Discovered                              ← Hours later
```

From a block time perspective, the chain appeared entirely normal after 08:47 — a steady block every 6 seconds. If your monitoring only tracks block production, you would believe the problem was resolved. But GRANDPA finality quietly died 15 minutes later, with **absolutely no block time anomalies to detect it**.

**Metrics that must be monitored simultaneously**:
- Block production rate / block time (failed to detect this incident)
- **Finality gap** (best block − finalised block) (**the only metric that could have detected this incident**)
- GRANDPA round state (prevotes/precommits weight)
- Peer count and sync state

```yaml
# Prometheus alerting rule example
- alert: FinalityGapTooLarge
  expr: substrate_block_height{status="best"} - substrate_block_height{status="finalized"} > 10
  for: 2m
  labels:
    severity: critical
  annotations:
    summary: "Finality gap exceeded 10 blocks — possible GRANDPA stall"
```

### Lesson 7: Observation windows must be long enough — don't draw conclusions during transition periods

Brief stalls after deploying a hotfix may be normal (migration executing, validators digesting new runtime/binary). However, that doesn't mean you should simply wait indefinitely. The correct approach is:

- Set a clear maximum waiting time (e.g. 5 minutes, 10 minutes)
- Continuously collect data during the waiting period (block height, finality gap, GRANDPA round state)
- If the time limit elapses without recovery → immediately execute the next remediation step (in this case, deploying the node binary)

---

## Action Items

### Immediate (P0)

- [x] Deploy new node binary to mainnet validators
- [x] Runtime upgrade to spec_version 94000004
- [x] Confirm finality restored and block production normal

### Short-term (this week)

- [ ] **Revert LongestChain hotfix**: Once finalisation has fully caught up and stabilised, switch `SelectRelayChain` back from `new_longest_chain` to `new_with_overseer` (the TODO in commit `381a78c51`)
- [ ] **K8s PDB**: Add a `PodDisruptionBudget` to validator StatefulSets, `minAvailable: 7`
- [ ] **Node key persistence**: Confirm all validator node keys are stored on PVs/Secrets
- [ ] **Parachain DB backups**: Establish a periodic backup mechanism

### Medium-term (this month)

- [ ] **Rolling update policy**: Enforce `RollingUpdate` strategy in K8s deployments; prohibit `Recreate`
- [ ] **Finality gap alerting (P0 priority)**: Establish a `best − finalised > 10` alert — this was the **only metric that could have detected this incident in time**
- [ ] **GRANDPA round monitoring**: Monitor prevotes/precommits weight; alert immediately on 0/10
- [ ] **Runbook**: Write a GRANDPA finality stall incident response runbook
- [ ] **Staging environment**: Run the complete upgrade + node binary deploy procedure on testnet before mainnet

### Long-term

- [ ] **srtool deterministic builds**: Use srtool to ensure runtime WASM reproducibility for verification
- [ ] **Automated canary deployments**: Upgrade 1 validator first, observe for 30 minutes, then upgrade the rest
- [ ] **Chaos testing**: Periodically simulate simultaneous validator restarts on testnet to verify system resilience

---

## Appendix: Key Files Changed

| File | Change | Purpose |
|------|--------|---------|
| `runtime/thxnet/src/lib.rs` | +151 lines | ParachainHost v4 API + GRANDPA migration + spec_version bump |
| `runtime/thxnet-testnet/src/lib.rs` | +20 lines | ParachainHost v4 API + spec_version bump |
| `node/service/src/lib.rs` | +5 −5 lines | SelectRelayChain → LongestChain hotfix |
| `.github/workflows/integration.yaml` | version bumps | Docker action upgrades for CI |
| `scripts/runtime-upgrade/force-runtime-upgrade.ts` | new file | Bun TypeScript runtime upgrade script |
