# fork-genesis cross-chain artifact contract

This document turns the current ad-hoc CI fallbacks into an explicit long-term
contract between **leafchains** (producer) and **rootchain** (consumer).

The immediate problem we just solved was real but narrow:

- self-hosted ARC runners did not reliably have the required leafchain inputs
- generic leafchains artifacts were *available* but did not carry the exact W6
  verification fixture semantics rootchain CI actually depended on
- rootchain therefore drifted into a mix of host-path assumptions, OCI fallbacks,
  and finally a repo-bundled full para spec to make the scenario deterministic

That stopgap was correct for shipping, but it is not yet the clean mechanism.

The long-term mechanism is: **reify cross-chain verification inputs as named,
versioned, immutable artifacts produced by leafchains and consumed by rootchain
through a small manifest contract**.

---

## 1. Design principles

### 1.1 Mechanism, not folklore

The workflow must not depend on:

- `/root/Works/...` or `/runner-assets/...` host paths
- a runner having previously built leafchains locally
- guessing that a generic `chain-specs/*.raw.json` happens to be equivalent to a
  specific verification fixture
- rebuilding leafchains in the rootchain verification workflow

### 1.2 Scenario-specific artifacts beat generic loose files

The failing run demonstrated that a generic raw chain spec was not equivalent to
our required verification fixture.

The consumer therefore must not reconstruct scenario semantics from loosely
related pieces unless the producer contract explicitly guarantees equivalence.

### 1.3 Bundle compatibility-critical inputs together

The leafchain binary and the verification para spec must come from the same
producer contract surface, so rootchain does not accidentally mix:

- binary from one leafchains commit
- spec from another commit
- wasm from a third source

### 1.4 Immutable references in CI

Rootchain CI should consume immutable digests, not floating branch tags, for the
actual verification run. Human-friendly tags may exist, but the workflow should
materialize a digest-pinned artifact once the target is selected.

---

## 2. Producer/consumer split

### Producer: `thxnet/leafchains`

Responsible for publishing **verification bundles** for named scenarios.

### Consumer: `thxnet/rootchain`

Responsible for selecting a verification scenario, materializing its bundle,
validating the manifest, and wiring the resulting assets into
`verify-cross-chain.sh` / `polkadot fork-genesis`.

---

## 3. Artifact taxonomy

## 3.1 Required published artifact classes

### A. Leafchain runtime binary artifact

Purpose: executable `thxnet-leafchain` for the scenario.

Recommended transport:

- OCI image or OCI artifact
- immutable digest pin

Minimum content:

- `/usr/local/bin/thxnet-leafchain`
- `/artifact-manifest.json`

### B. Cross-chain verification bundle artifact

Purpose: carry the **scenario-specific** inputs rootchain needs to register and
boot the para deterministically.

Recommended transport:

- OCI artifact image
- immutable digest pin

Minimum content:

- `/bundle/manifest.json`
- `/bundle/para-spec.raw.json`
- `/bundle/validation-code.wasm`
- `/bundle/genesis-state.bin`
- optional `/bundle/notes.md`

Important:

- `para-spec.raw.json` is the canonical input for rootchain’s current
  `--register-leafchain=<paraId>:<spec>` contract
- `validation-code.wasm` and `genesis-state.bin` are included for verification,
  provenance, future contract evolution, and debugging
- rootchain should treat `para-spec.raw.json` as authoritative for today’s flow

### C. Optional compatibility alias artifact

Purpose: human-friendly moving tag, e.g. latest good scenario bundle.

Examples:

- `ghcr.io/thxnet/leafchain-verification/w6-t3-verify:latest`
- `ghcr.io/thxnet/leafchain-verification/w6-t3-verify:git-<sha>`

But the rootchain workflow should resolve these to a digest before executing.

---

## 4. Scenario model

A verification bundle is not “the sand chain” in the abstract.
It is **a named verification scenario**.

Examples:

- `w6-t3-verify`
- `sand-testnet-smoke`
- `runtime-upgrade-rehearsal-2026-04-19`

Each scenario must declare:

- `scenario_id`
- `leafchain_id` / `para_id`
- `para_chain_id` (for example `sand_testnet`)
- intended relay chain (`thxnet-testnet` / `thxnet-mainnet`)
- producer repo commit (`leafchains_git_sha`)
- semantic purpose (`verify-cross-chain`, `upgrade-rehearsal`, etc.)

This avoids pretending a generic chain spec is interchangeable with a scenario
fixture.

---

## 5. Manifest contract

Every published verification bundle must contain a machine-readable manifest.

## 5.1 Example `manifest.json`

```json
{
  "schema_version": 1,
  "artifact_kind": "leafchain-cross-chain-verification-bundle",
  "scenario_id": "w6-t3-verify",
  "relay_chain": "thxnet-testnet",
  "para_id": 1003,
  "para_chain_id": "sand_testnet",
  "leafchain_name": "Sandbox",
  "leafchains_git_sha": "3267d1cdefb...",
  "leafchain_binary": {
    "image": "ghcr.io/thxnet/leafchain@sha256:<digest>",
    "version": "0.3.3-3267d1cdefb"
  },
  "paths": {
    "para_spec": "/bundle/para-spec.raw.json",
    "validation_code": "/bundle/validation-code.wasm",
    "genesis_state": "/bundle/genesis-state.bin"
  },
  "checksums": {
    "para_spec_sha256": "...",
    "validation_code_sha256": "...",
    "genesis_state_sha256": "..."
  },
  "rootchain_contract": {
    "register_leafchain_mode": "spec-json",
    "verify_cross_chain_min_version": 1
  }
}
```

## 5.2 Consumer validation rules

Before rootchain uses a bundle, it must validate:

1. `schema_version` is supported
2. `artifact_kind` is expected
3. `relay_chain`, `para_id`, and `para_chain_id` match workflow inputs
4. referenced files exist in the bundle
5. declared checksums match extracted files
6. if a binary image ref is present, its digest is materialized exactly

If any of these fail, the workflow should stop early with a contract error,
not continue with best-effort guessing.

---

## 6. Rootchain consumer contract

## 6.1 Workflow surface

Replace today’s loosely coupled fallback inputs with a more explicit interface:

### Preferred long-term workflow inputs

- `verification_bundle_ref`
  - OCI ref for the scenario bundle, ideally digest-pinned
- `leafchain_binary_ref`
  - optional override; if omitted, use the manifest’s binary ref
- `relay_chain`
- `register_para_id`
- `para_chain_id`

### Deprecated compatibility inputs

Keep temporarily but phase out:

- `leafchain_bin`
- `leafchain_image`
- `genesis_image`
- `para_json`

These are useful escape hatches during migration, but not the final contract.

## 6.2 Materialization flow

Rootchain workflow should do this:

1. pull `verification_bundle_ref`
2. extract `/bundle/manifest.json`
3. validate the manifest
4. extract `para-spec.raw.json`, `validation-code.wasm`, `genesis-state.bin`
5. resolve `leafchain_binary_ref`:
   - explicit input override, else manifest-provided ref
6. materialize `thxnet-leafchain`
7. run `verify-cross-chain.sh` with:
   - `VERIFY_CROSS_CHAIN_LEAFCHAIN_BIN`
   - `VERIFY_CROSS_CHAIN_PARA_JSON`
   - existing relay binary and seed-db inputs

## 6.3 Current repo-bundled full spec becomes an emergency fallback only

The current `scripts/fork-genesis/assets/w6-t3-verify.json.xz` is acceptable as
an operational stopgap. Long-term it should move out of rootchain and become a
producer-owned published verification bundle.

Rootchain may keep the bundled asset for disaster recovery, but it should no
longer be the primary mechanism once the producer contract exists.

---

## 7. Leafchains producer contract

Leafchains should gain a producer step that emits scenario bundles explicitly.

## 7.1 Producer responsibilities

For each named scenario, leafchains must be able to:

1. build the exact `thxnet-leafchain` binary
2. generate or materialize the exact para spec fixture for that scenario
3. export validation code wasm
4. export genesis state
5. generate `manifest.json`
6. publish bundle + binary as immutable OCI references

## 7.2 Producer CLI shape

Suggested producer entrypoint shape:

```bash
# examples only
thxnet-leafchain build-verification-bundle \
  --scenario-id=w6-t3-verify \
  --chain=sand_testnet \
  --para-id=1003 \
  --output-dir=dist/w6-t3-verify
```

Or a repo-local script:

```bash
scripts/release/build-verification-bundle.sh w6-t3-verify
```

The important part is not the exact command; it is that the output contract is
stable and machine-readable.

---

## 8. Why the current generic genesis-image spec was insufficient

This should be made explicit because it is the crux of the recent incident.

Observed facts:

- generic genesis-image raw spec size was ~1.7 MB
- successful W6 verification spec was ~134 MB
- the smaller generic spec allowed the workflow to reach the runtime phase but
  did not yield para block #1 in the verification scenario
- the full W6 spec did

Therefore rootchain must not assume:

> “any raw spec for `sand_testnet` is equivalent to the verification fixture”

The producer contract must encode scenario semantics, not only chain identity.

---

## 9. Migration plan

## Wave 1 — stabilize current rootchain PR

Status: effectively done.

- rootchain workflow no longer depends solely on runner host paths
- binary/spec/seed inputs can be materialized in CI
- repo-bundled full W6 spec closes the immediate correctness gap

## Wave 2 — publish first-class verification bundles from leafchains

Deliverables:

- leafchains producer script/command for `w6-t3-verify`
- OCI-published verification bundle
- manifest schema v1
- digest-pinned refs recorded somewhere operator-friendly

Success criteria:

- rootchain can consume the external bundle and pass cross-chain verification
  with the repo-bundled fallback disabled in a rehearsal run

## Wave 3 — make bundle consumption the default rootchain path

Deliverables:

- rootchain workflow prefers `verification_bundle_ref`
- existing loose inputs become deprecated compatibility knobs
- `README.md` / `HANDOFF.md` updated to point at the artifact contract

Success criteria:

- new runs no longer rely on `scripts/fork-genesis/assets/w6-t3-verify.json.xz`
  in the normal path

## Wave 4 — remove accidental contract surfaces

Deliverables:

- stop documenting `/root/Works/leafchains/...` as a normal CI dependency
- stop treating generic `/chain-specs/*.raw.json` as equivalent to a scenario
  verification fixture
- keep only explicit scenario bundles plus emergency escape hatches

---

## 10. Acceptance criteria for the long-term contract

The contract is successful when all of the following are true:

1. rootchain cross-chain CI passes on a fresh ARC runner with no pre-provisioned
   leafchains repo checkout
2. rootchain does not need to build leafchains in-workflow
3. rootchain does not need repo-bundled scenario fixtures in the normal path
4. binary/spec/wasm provenance all point back to one scenario manifest
5. operators can identify exactly which leafchains commit produced the consumed
   scenario
6. a future scenario (not only `w6-t3-verify`) can be added without editing
   rootchain workflow logic beyond choosing a different bundle ref

---

## 11. Immediate next implementation tasks

1. **Leafchains**: add a producer script/CLI that emits a scenario bundle for
   `w6-t3-verify`
2. **Leafchains**: publish that bundle to GHCR with a manifest
3. **Rootchain**: teach `.github/workflows/fork-genesis-cross-chain.yaml` to
   consume `verification_bundle_ref`
4. **Rootchain**: keep the current bundled full spec as emergency fallback only
5. **Rootchain**: add a rehearsal mode or dispatch input that disables the local
   bundled fallback so the new external contract is tested for real

---

## 12. Non-goals

This contract does **not** require rootchain to understand every detail of how
leafchains generated the scenario.

That policy belongs in the producer.

The consumer only needs:

- a stable manifest schema
- stable file paths inside the bundle
- checksum verification
- a small set of scenario identity fields

That is the whole point of the boundary.
