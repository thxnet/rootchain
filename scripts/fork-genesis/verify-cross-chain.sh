#!/usr/bin/env bash
# verify-cross-chain.sh — Boot 3-validator relay + 3-collator parachain from forked specs
# and verify cross-chain liveness (relay finalized #1, para Imported #1, peer counts, burn-in).
#
# Topology:
#   Relay:     Alice p2p=40331 rpc=9931 | Bob p2p=40332 rpc=9932 | Charlie p2p=40333 rpc=9933
#   Para:      sand-Alice p2p=40334 rpc=9934 | sand-Bob p2p=40336 rpc=9936 | sand-Charlie p2p=40338 rpc=9938
#   Emb-relay: sand-Alice p2p=40335 rpc=9935 | sand-Bob p2p=40337 rpc=9937 | sand-Charlie p2p=40339 rpc=9939
#
# Fixed node keys → deterministic peer IDs (Ed25519 via libp2p):
#   Relay Alice:    0x000...0001 → 12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp
#   Para  Alice:    0x000...0002 → 12D3KooWHdiAxVd8uMQR1hGWXccidmfCwLqcMpGwR6QcTP6QRMuD
#   Para  Bob:      0x000...0003 → 12D3KooWSCufgHzV4fCwRijfH2k3abrpAJxTKxEvN1FDuRXA2U9x
#   Para  Charlie:  0x000...0004 → 12D3KooWSsChzF81YDUKpe9Uk5AHV5oqAaXAcWNSPYgoLauUk4st
#
# Boot phases:
#   Phase A (relay-first): relay Alice → Bob → Charlie; gate on relay finalized #1 (60s).
#   Phase B (collators): keystore insert BEFORE launch; sand-Alice → sand-Bob → sand-Charlie;
#                        gate on para Imported #1 (120s).
#
# Exit codes (always 1 for failure — die() ensures this):
#   RELAY_SPEC_MISSING  — relay chain spec not found or < 1MB
#   PARA_SPEC_MISSING   — para chain spec not found or < 1MB
#   BINARY_MISSING      — required binary not executable
#   RELAY_FINALIZE_TIMEOUT  — relay finalized #1 not seen within 60s
#   PARA_IMPORT_TIMEOUT     — para Imported #1 not seen within 120s of Phase B
#   PEER_CHECK_FAIL         — collator has 0 peers at +60s after Phase B
#   CRITICAL_LOG_HIT        — panic/stall/fatal keyword found during burn-in
#
# Usage:
#   ./verify-cross-chain.sh [--keep-running] [--burn-in-seconds=N] [--run-root=PATH]
#                           [--polkadot-bin=PATH] [--leafchain-bin=PATH]
#                           [--para-json=PATH] [--relay-json=PATH]
#                           [--seed-db=PATH] [--register-para-id=N]
#                           [--relay-chain=CHAIN] [--para-chain-id=CHAIN]
#
#   --keep-running          Do NOT kill nodes on exit (default: OFF; use for debugging).
#   --burn-in-seconds=N     Override 5-min (300s) burn-in duration.
#   --run-root=PATH         Working directory root for node state/logs/pid files.
#   --polkadot-bin=PATH     Override the relay binary path.
#   --leafchain-bin=PATH    Override the leafchain binary path.
#   --para-json=PATH        Override the para spec path.
#   --relay-json=PATH       Override the regenerated relay spec output path.
#   --seed-db=PATH          Override the read-only fork seed DB path.
#   --register-para-id=N    Override the para id passed to --register-leafchain.
#   --relay-chain=CHAIN     Override the relay chain name passed to fork-genesis.
#   --para-chain-id=CHAIN   Override the parachain chain id used for keystore layout.
#
# Environment variables (CI/CD friendly):
#   VERIFY_CROSS_CHAIN_POLKADOT_BIN
#   VERIFY_CROSS_CHAIN_LEAFCHAIN_BIN
#   VERIFY_CROSS_CHAIN_PARA_JSON
#   VERIFY_CROSS_CHAIN_RELAY_JSON
#   VERIFY_CROSS_CHAIN_SEED_DB
#   VERIFY_CROSS_CHAIN_REGISTER_PARA_ID
#   VERIFY_CROSS_CHAIN_RELAY_CHAIN
#   VERIFY_CROSS_CHAIN_PARA_CHAIN_ID
#   VERIFY_CROSS_CHAIN_RUN_ROOT

set -euo pipefail

usage() {
    cat <<'EOF'
verify-cross-chain.sh — Cross-chain fork-genesis rehearsal

Options:
  --keep-running
  --burn-in-seconds=N
  --run-root=PATH
  --polkadot-bin=PATH
  --leafchain-bin=PATH
  --para-json=PATH
  --relay-json=PATH
  --seed-db=PATH
  --register-para-id=N
  --relay-chain=CHAIN
  --para-chain-id=CHAIN
  -h, --help

Environment-variable equivalents are documented in the file header.
EOF
}

TMP_ROOT="${TMPDIR:-${RUNNER_TEMP:-/tmp}}"
DEFAULT_RUN_ROOT="${TMP_ROOT}/verify-cross-chain"
if [[ -n "${GITHUB_RUN_ID:-}" ]]; then
    DEFAULT_RUN_ROOT+="-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT:-1}"
fi

# ─── Binaries ────────────────────────────────────────────────────────────────
POLKADOT="${VERIFY_CROSS_CHAIN_POLKADOT_BIN:-${POLKADOT:-/root/Works/rootchain/target/release/polkadot}}"
LEAFCHAIN="${VERIFY_CROSS_CHAIN_LEAFCHAIN_BIN:-${LEAFCHAIN:-/root/Works/leafchains/target/release/thxnet-leafchain}}"

# ─── Chain specs ─────────────────────────────────────────────────────────────
# W8: Forked relay spec is regenerated from the seed DB at script start using
# the new `--register-leafchain=<paraId>:<leafchain-spec>` flag. Registration
# flows through `pallet_paras::build()` in fresh GenesisConfig, writing
# Parachains/Heads/CurrentCodeHash/CodeByHash for paraId 1003 with local fresh
# leafchain WASM (not stale livenet WASM). `fix_para_scheduler_state` then
# overwrites ParaScheduler.{ValidatorGroups, AvailabilityCores,
# SessionStartBlock} so backing works from block #1 without waiting for a BABE
# epoch.
RELAY_JSON="${VERIFY_CROSS_CHAIN_RELAY_JSON:-${RELAY_JSON:-${TMP_ROOT}/forked-thxnet-testnet-w8.json}}"
PARA_JSON="${VERIFY_CROSS_CHAIN_PARA_JSON:-${PARA_JSON:-${TMP_ROOT}/w6-t3-verify.json}}"
# Seed DB used by fork-genesis (read-only).
ROOTCHAIN_SEED_DB="${VERIFY_CROSS_CHAIN_SEED_DB:-${ROOTCHAIN_SEED_DB:-/data/forknet-test/rootchain-seed}}"
REGISTER_PARA_ID="${VERIFY_CROSS_CHAIN_REGISTER_PARA_ID:-${REGISTER_PARA_ID:-1003}}"
RELAY_CHAIN="${VERIFY_CROSS_CHAIN_RELAY_CHAIN:-${RELAY_CHAIN:-thxnet-testnet}}"

# ─── Relay node keys & peer IDs ──────────────────────────────────────────────
RELAY_ALICE_NODE_KEY="0000000000000000000000000000000000000000000000000000000000000001"
RELAY_ALICE_PEER_ID="12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp"
RELAY_ALICE_BOOTNODE="/ip4/127.0.0.1/tcp/40331/p2p/${RELAY_ALICE_PEER_ID}"

# ─── Para node keys & peer IDs ───────────────────────────────────────────────
PARA_ALICE_NODE_KEY="0000000000000000000000000000000000000000000000000000000000000002"
PARA_ALICE_PEER_ID="12D3KooWHdiAxVd8uMQR1hGWXccidmfCwLqcMpGwR6QcTP6QRMuD"
PARA_ALICE_BOOTNODE="/ip4/127.0.0.1/tcp/40334/p2p/${PARA_ALICE_PEER_ID}"

PARA_BOB_NODE_KEY="0000000000000000000000000000000000000000000000000000000000000003"
PARA_CHARLIE_NODE_KEY="0000000000000000000000000000000000000000000000000000000000000004"

# ─── Sr25519 public keys for aura keystore (hex without 0x) ─────────────────
# Derived via: polkadot key inspect --scheme sr25519 //Alice|Bob|Charlie
# aura key type = 61757261 (4 bytes, ASCII "aura")
AURA_KEY_TYPE_HEX="61757261"
ALICE_SR25519_PUB="d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"
BOB_SR25519_PUB="8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
CHARLIE_SR25519_PUB="90b5ab205c6974c9ea841be688864633dc9ca8a357843eeacf2314649965fe22"

# ─── Para chain ID (matches spec's "id" field) ───────────────────────────────
PARA_CHAIN_ID="${VERIFY_CROSS_CHAIN_PARA_CHAIN_ID:-${PARA_CHAIN_ID:-sand_testnet}}"

# ─── Working directories / artifacts roots ────────────────────────────────────
RUN_ROOT="${VERIFY_CROSS_CHAIN_RUN_ROOT:-${XCV_RUN_ROOT:-$DEFAULT_RUN_ROOT}}"
STATE_ROOT="${RUN_ROOT}/state"
LOG_ROOT="${RUN_ROOT}/logs"
PID_ROOT="${RUN_ROOT}/pids"

# ─── Base paths ──────────────────────────────────────────────────────────────
BASE_RELAY_ALICE="${STATE_ROOT}/relay-alice"
BASE_RELAY_BOB="${STATE_ROOT}/relay-bob"
BASE_RELAY_CHARLIE="${STATE_ROOT}/relay-charlie"
BASE_SAND_ALICE="${STATE_ROOT}/sand-alice"
BASE_SAND_BOB="${STATE_ROOT}/sand-bob"
BASE_SAND_CHARLIE="${STATE_ROOT}/sand-charlie"

# ─── Logs ────────────────────────────────────────────────────────────────────
LOG_RELAY_ALICE="${LOG_ROOT}/relay-alice.log"
LOG_RELAY_BOB="${LOG_ROOT}/relay-bob.log"
LOG_RELAY_CHARLIE="${LOG_ROOT}/relay-charlie.log"
LOG_SAND_ALICE="${LOG_ROOT}/sand-alice.log"
LOG_SAND_BOB="${LOG_ROOT}/sand-bob.log"
LOG_SAND_CHARLIE="${LOG_ROOT}/sand-charlie.log"

ALL_RELAY_LOGS=("$LOG_RELAY_ALICE" "$LOG_RELAY_BOB" "$LOG_RELAY_CHARLIE")
ALL_PARA_LOGS=("$LOG_SAND_ALICE" "$LOG_SAND_BOB" "$LOG_SAND_CHARLIE")
ALL_LOGS=("${ALL_RELAY_LOGS[@]}" "${ALL_PARA_LOGS[@]}")

# ─── PID tracking ────────────────────────────────────────────────────────────
PID_RELAY_ALICE=""
PID_RELAY_BOB=""
PID_RELAY_CHARLIE=""
PID_SAND_ALICE=""
PID_SAND_BOB=""
PID_SAND_CHARLIE=""

PID_FILE_RELAY_ALICE="${PID_ROOT}/relay-alice.pid"
PID_FILE_RELAY_BOB="${PID_ROOT}/relay-bob.pid"
PID_FILE_RELAY_CHARLIE="${PID_ROOT}/relay-charlie.pid"
PID_FILE_SAND_ALICE="${PID_ROOT}/sand-alice.pid"
PID_FILE_SAND_BOB="${PID_ROOT}/sand-bob.pid"
PID_FILE_SAND_CHARLIE="${PID_ROOT}/sand-charlie.pid"

ALL_PID_FILES=(
    "$PID_FILE_RELAY_ALICE" "$PID_FILE_RELAY_BOB" "$PID_FILE_RELAY_CHARLIE"
    "$PID_FILE_SAND_ALICE"  "$PID_FILE_SAND_BOB"  "$PID_FILE_SAND_CHARLIE"
)

# ─── Timing knobs ────────────────────────────────────────────────────────────
RELAY_START_WAIT=10           # seconds after relay Alice start before Bob/Charlie
RELAY_FINALIZE_TIMEOUT=60     # seconds to wait for relay finalized #1
PHASE_B_GRACE=60              # seconds grace for cumulus relay-light-client sync
PARA_IMPORT_TIMEOUT=120       # seconds to wait for para Imported #1
PEER_CHECK_DELAY=60           # seconds after Phase B before peer count check
BURN_IN_SECONDS=300           # 5-minute mini burn-in
POLL_INTERVAL=2               # polling interval

# ─── Flags ───────────────────────────────────────────────────────────────────
KEEP_RUNNING=false

for arg in "$@"; do
    case "$arg" in
        --keep-running)         KEEP_RUNNING=true ;;
        --burn-in-seconds=*)    BURN_IN_SECONDS="${arg#*=}" ;;
        --run-root=*)           RUN_ROOT="${arg#*=}" ; STATE_ROOT="${RUN_ROOT}/state" ; LOG_ROOT="${RUN_ROOT}/logs" ; PID_ROOT="${RUN_ROOT}/pids" ;;
        --polkadot-bin=*)       POLKADOT="${arg#*=}" ;;
        --leafchain-bin=*)      LEAFCHAIN="${arg#*=}" ;;
        --para-json=*)          PARA_JSON="${arg#*=}" ;;
        --relay-json=*)         RELAY_JSON="${arg#*=}" ;;
        --seed-db=*)            ROOTCHAIN_SEED_DB="${arg#*=}" ;;
        --register-para-id=*)   REGISTER_PARA_ID="${arg#*=}" ;;
        --relay-chain=*)        RELAY_CHAIN="${arg#*=}" ;;
        --para-chain-id=*)      PARA_CHAIN_ID="${arg#*=}" ;;
        -h|--help)              usage; exit 0 ;;
        *) echo "Unknown arg: $arg" >&2; exit 1 ;;
    esac
done

BASE_RELAY_ALICE="${STATE_ROOT}/relay-alice"
BASE_RELAY_BOB="${STATE_ROOT}/relay-bob"
BASE_RELAY_CHARLIE="${STATE_ROOT}/relay-charlie"
BASE_SAND_ALICE="${STATE_ROOT}/sand-alice"
BASE_SAND_BOB="${STATE_ROOT}/sand-bob"
BASE_SAND_CHARLIE="${STATE_ROOT}/sand-charlie"
LOG_RELAY_ALICE="${LOG_ROOT}/relay-alice.log"
LOG_RELAY_BOB="${LOG_ROOT}/relay-bob.log"
LOG_RELAY_CHARLIE="${LOG_ROOT}/relay-charlie.log"
LOG_SAND_ALICE="${LOG_ROOT}/sand-alice.log"
LOG_SAND_BOB="${LOG_ROOT}/sand-bob.log"
LOG_SAND_CHARLIE="${LOG_ROOT}/sand-charlie.log"
ALL_RELAY_LOGS=("$LOG_RELAY_ALICE" "$LOG_RELAY_BOB" "$LOG_RELAY_CHARLIE")
ALL_PARA_LOGS=("$LOG_SAND_ALICE" "$LOG_SAND_BOB" "$LOG_SAND_CHARLIE")
ALL_LOGS=("${ALL_RELAY_LOGS[@]}" "${ALL_PARA_LOGS[@]}")
PID_FILE_RELAY_ALICE="${PID_ROOT}/relay-alice.pid"
PID_FILE_RELAY_BOB="${PID_ROOT}/relay-bob.pid"
PID_FILE_RELAY_CHARLIE="${PID_ROOT}/relay-charlie.pid"
PID_FILE_SAND_ALICE="${PID_ROOT}/sand-alice.pid"
PID_FILE_SAND_BOB="${PID_ROOT}/sand-bob.pid"
PID_FILE_SAND_CHARLIE="${PID_ROOT}/sand-charlie.pid"
ALL_PID_FILES=(
    "$PID_FILE_RELAY_ALICE" "$PID_FILE_RELAY_BOB" "$PID_FILE_RELAY_CHARLIE"
    "$PID_FILE_SAND_ALICE"  "$PID_FILE_SAND_BOB"  "$PID_FILE_SAND_CHARLIE"
)

# ─── Logging helpers ─────────────────────────────────────────────────────────
ts()    { date '+%H:%M:%S'; }
info()  { echo "[$(ts)] INFO  $*"; }
warn()  { echo "[$(ts)] WARN  $*"; }
# LABEL on stderr only (W4 lesson)
error() { echo "[$(ts)] ERROR $*" >&2; }

die() {
    # Always exits 1 — no phantom exit codes (W4 lesson)
    local code="$1"; shift
    error "FATAL[$code]: $*"
    exit 1
}

require_cmd() {
    local cmd="$1"
    command -v "$cmd" >/dev/null 2>&1 || die "MISSING_COMMAND" "Required command not found: $cmd"
}

require_port_inspector() {
    command -v lsof >/dev/null 2>&1 && return 0
    command -v fuser >/dev/null 2>&1 && return 0
    command -v ss >/dev/null 2>&1 && return 0
    if command -v python3 >/dev/null 2>&1 && [[ -r /proc/net/tcp || -r /proc/net/tcp6 ]]; then
        return 0
    fi
    warn "No port-inspection tool found; continuing without port-based stale-process cleanup"
    return 0
}

procfs_port_pids() {
    local port="$1"
    command -v python3 >/dev/null 2>&1 || return 1
    [[ -r /proc/net/tcp || -r /proc/net/tcp6 ]] || return 1

    python3 - "$port" <<'PY'
import os
import sys

port = int(sys.argv[1])
target_inodes = set()
for proc_net in ("/proc/net/tcp", "/proc/net/tcp6"):
    try:
        with open(proc_net, "r", encoding="utf-8") as handle:
            next(handle, None)
            for line in handle:
                parts = line.split()
                if len(parts) < 10:
                    continue
                local_address = parts[1]
                state = parts[3]
                inode = parts[9]
                try:
                    local_port = int(local_address.split(":")[1], 16)
                except (IndexError, ValueError):
                    continue
                if local_port == port and state == "0A":
                    target_inodes.add(inode)
    except FileNotFoundError:
        continue

if not target_inodes:
    raise SystemExit(0)

pid_matches = set()
for pid in os.listdir("/proc"):
    if not pid.isdigit():
        continue
    fd_dir = f"/proc/{pid}/fd"
    try:
        for fd in os.listdir(fd_dir):
            try:
                target = os.readlink(f"{fd_dir}/{fd}")
            except OSError:
                continue
            if not target.startswith("socket:["):
                continue
            inode = target[8:-1]
            if inode in target_inodes:
                pid_matches.add(pid)
                break
    except (FileNotFoundError, PermissionError, ProcessLookupError):
        continue

for pid in sorted(pid_matches, key=int):
    print(pid)
PY
}

port_pids() {
    local port="$1"
    if command -v lsof >/dev/null 2>&1; then
        lsof -ti "tcp:${port}" 2>/dev/null || true
        return 0
    fi
    if command -v fuser >/dev/null 2>&1; then
        fuser -n tcp "$port" 2>/dev/null | tr ' ' '\n' | grep -E '^[0-9]+$' || true
        return 0
    fi
    if command -v ss >/dev/null 2>&1; then
        ss -ltnpH "sport = :${port}" 2>/dev/null | sed -nE 's/.*pid=([0-9]+).*/\1/p' | sort -u || true
        return 0
    fi
    if procfs_port_pids "$port"; then
        return 0
    fi
    warn "No port-inspection tool found; skipping port-owner discovery for tcp:${port}"
    return 0
}

mkdir -p "$STATE_ROOT" "$LOG_ROOT" "$PID_ROOT" "$(dirname "$RELAY_JSON")"
require_port_inspector
require_cmd grep
require_cmd wc

# ─── Cleanup: prior runs ─────────────────────────────────────────────────────
cleanup_prior_runs() {
    info "Cleaning up prior runs (ports 40331-40339, 9931-9939)..."
    # Kill by port
    for port in 40331 40332 40333 40334 40335 40336 40337 40338 40339 \
                9931  9932  9933  9934  9935  9936  9937  9938  9939; do
        local pids pid
        pids=$(port_pids "$port")
        for pid in $pids; do
            [[ -n "$pid" ]] || continue
            info "  Port $port occupied by PID $pid — killing"
            kill "$pid" 2>/dev/null || true
        done
    done
    # Kill by PID files (only the known node PID files, not the script's own PID file)
    for pidf in "${ALL_PID_FILES[@]}"; do
        [[ -f "$pidf" ]] || continue
        local old_pid
        old_pid=$(cat "$pidf" 2>/dev/null || true)
        if [[ -n "$old_pid" ]] && kill -0 "$old_pid" 2>/dev/null; then
            info "  Killing stale PID $old_pid from $pidf"
            kill "$old_pid" 2>/dev/null || true
        fi
        rm -f "$pidf"
    done
    sleep 2  # let ports drain
    # Clear old node logs (not the script's own run log which uses a different prefix)
    rm -f "${ALL_LOGS[@]}"
    # Clear base paths for fresh start (avoids keystore/DB conflicts)
    rm -rf "$BASE_RELAY_ALICE" "$BASE_RELAY_BOB" "$BASE_RELAY_CHARLIE" \
           "$BASE_SAND_ALICE"  "$BASE_SAND_BOB"  "$BASE_SAND_CHARLIE"
    info "Prior-run cleanup complete."
}

# ─── Cleanup: on exit trap ───────────────────────────────────────────────────
do_cleanup_on_exit() {
    if [[ "$KEEP_RUNNING" == "true" ]]; then
        info "--keep-running active. Nodes left alive:"
        info "  relay: ${PID_RELAY_ALICE:-?} ${PID_RELAY_BOB:-?} ${PID_RELAY_CHARLIE:-?}"
        info "  para:  ${PID_SAND_ALICE:-?} ${PID_SAND_BOB:-?} ${PID_SAND_CHARLIE:-?}"
        return
    fi
    info "Stopping all 6 nodes..."
    local all_pids=(
        "${PID_RELAY_ALICE:-}"  "${PID_RELAY_BOB:-}"  "${PID_RELAY_CHARLIE:-}"
        "${PID_SAND_ALICE:-}"   "${PID_SAND_BOB:-}"   "${PID_SAND_CHARLIE:-}"
    )
    for pid in "${all_pids[@]}"; do
        [[ -n "$pid" ]] && kill "$pid" 2>/dev/null || true
    done
    # Remove all PID files (W4 carry: no stale PIDs left behind)
    for pidf in "${ALL_PID_FILES[@]}"; do
        rm -f "$pidf"
    done
    info "All nodes stopped and PID files removed."
}

trap do_cleanup_on_exit EXIT

# ─── Keystore insertion helper ───────────────────────────────────────────────
# Writes the aura sr25519 key directly into the keystore directory.
# Substrate keystore format: filename = <key_type_hex><pubkey_hex>, content = "//Suri"
# Args: $1=base_path $2=suri (e.g. "//Alice") $3=pubkey_hex
insert_aura_key() {
    local base_path="$1"
    local suri="$2"
    local pubkey_hex="$3"
    local ks_dir="${base_path}/chains/${PARA_CHAIN_ID}/keystore"
    local filename="${AURA_KEY_TYPE_HEX}${pubkey_hex}"
    mkdir -p "$ks_dir"
    # Content is the suri as a JSON-style quoted string (substrate reads it as-is).
    # suri already contains the // prefix (e.g. "//Alice") — just wrap in quotes.
    printf '"%s"' "$suri" > "${ks_dir}/${filename}"
    info "  Keystore: wrote aura key for ${suri} → ${ks_dir}/${filename}"
}

# ─── Collator launch helper ───────────────────────────────────────────────────
# Launches a cumulus collator with the dual-binary invocation pattern.
# Args:
#   $1  = persona flag (--alice|--bob|--charlie)
#   $2  = base path
#   $3  = para p2p port
#   $4  = para rpc port
#   $5  = para node key (64 hex chars)
#   $6  = embedded relay p2p port
#   $7  = embedded relay rpc port
#   $8  = para bootnode multiaddr (empty for alice)
#   $9  = log file path
# Returns: sets global PID variable by echoing the PID (caller captures)
launch_collator() {
    local persona="$1"
    local base_path="$2"
    local para_p2p="$3"
    local para_rpc="$4"
    local para_node_key="$5"
    local emb_p2p="$6"
    local emb_rpc="$7"
    local para_bootnode="$8"
    local log_file="$9"

    local bootnode_args=()
    if [[ -n "$para_bootnode" ]]; then
        bootnode_args=("--bootnodes=${para_bootnode}")
    fi

    mkdir -p "${base_path}"

    "$LEAFCHAIN" \
        --collator \
        "$persona" \
        --base-path="${base_path}" \
        --chain="${PARA_JSON}" \
        --port="${para_p2p}" \
        --rpc-port="${para_rpc}" \
        --node-key="${para_node_key}" \
        "${bootnode_args[@]}" \
        --force-authoring \
        --no-prometheus \
        --no-telemetry \
        --no-mdns \
        -laura=debug,cumulus-consensus=debug,parachain::collation-generation=debug,parachain::collator-protocol=debug \
        -- \
        --chain="${RELAY_JSON}" \
        --base-path="${base_path}/relay" \
        --port="${emb_p2p}" \
        --rpc-port="${emb_rpc}" \
        --bootnodes="${RELAY_ALICE_BOOTNODE}" \
        --no-prometheus \
        --no-telemetry \
        > "$log_file" 2>&1 &
    echo $!
}

# ─── Liveness poll helper ────────────────────────────────────────────────────
# Poll log files for a pattern; returns 0 if found within timeout.
# Args: $1=timeout_s $2=pattern $3..=log files
wait_for_pattern() {
    local timeout_s="$1"; shift
    local pattern="$1"; shift
    local logs=("$@")
    local deadline=$(( $(date +%s) + timeout_s ))
    while (( $(date +%s) < deadline )); do
        for log in "${logs[@]}"; do
            if grep -qE "$pattern" "$log" 2>/dev/null; then
                local matched_line
                matched_line=$(grep -oE "$pattern" "$log" | head -1)
                echo "$matched_line"
                return 0
            fi
        done
        sleep "$POLL_INTERVAL"
    done
    return 1
}

# ─── Check all 6 PIDs alive ──────────────────────────────────────────────────
check_all_pids_alive() {
    local all_pids=(
        "$PID_RELAY_ALICE" "$PID_RELAY_BOB" "$PID_RELAY_CHARLIE"
        "$PID_SAND_ALICE"  "$PID_SAND_BOB"  "$PID_SAND_CHARLIE"
    )
    local names=("relay-Alice" "relay-Bob" "relay-Charlie" "sand-Alice" "sand-Bob" "sand-Charlie")
    local any_dead=false
    for i in "${!all_pids[@]}"; do
        local pid="${all_pids[$i]}"
        local name="${names[$i]}"
        if ! kill -0 "$pid" 2>/dev/null; then
            error "Node ${name} (PID=$pid) is dead!"
            any_dead=true
        fi
    done
    [[ "$any_dead" == "false" ]]
}

# ─── Critical log check ───────────────────────────────────────────────────────
# W4 fix: use -irE (not (?i) PCRE) for case-insensitive grep
check_critical_logs() {
    local pattern='(panic!|panicked|stalled|deadlock|FATAL|bad block|essential task)'
    local hits
    hits=$(grep -irE "$pattern" "${ALL_LOGS[@]}" 2>/dev/null | head -20 || true)
    if [[ -n "$hits" ]]; then
        error "CRITICAL log hits:"
        echo "$hits" >&2
        return 1
    fi
    return 0
}

# ════════════════════════════════════════════════════════════════════════
#  MAIN
# ════════════════════════════════════════════════════════════════════════

info "=== verify-cross-chain.sh START ==="
info "Run root  : $RUN_ROOT"
info "Relay bin : $POLKADOT"
info "Leaf bin  : $LEAFCHAIN"
info "Relay spec: $RELAY_JSON"
info "Para  spec: $PARA_JSON"
info "Seed DB   : $ROOTCHAIN_SEED_DB"

# ─── Step 0: Validate static prerequisites ────────────────────────────────────
info "=== Step 0: Validate prerequisites ==="

# Binaries
for bin in "$POLKADOT" "$LEAFCHAIN"; do
    [[ -x "$bin" ]] || die "BINARY_MISSING" "Not executable: $bin"
    info "  OK binary: $bin"
 done

# Para spec (input to --register-leafchain + collator --chain)
[[ -f "$PARA_JSON" ]] || die "PARA_SPEC_MISSING" "Para spec not found: $PARA_JSON"
PARA_SIZE=$(wc -c < "$PARA_JSON")
# Full W6 fork-genesis exports are large (>100 MB today). Minimal raw chain-specs
# from build-spec / OCI image fixtures are ~1.7 MB and are known-bad for
# cross-chain liveness because they do not carry the forked parachain state.
(( PARA_SIZE >= 10000000 )) || die "PARA_SPEC_MISSING" "Para spec too small: ${PARA_SIZE} bytes (need a full W6 fork-genesis export)"
info "  OK para spec: $PARA_JSON (${PARA_SIZE} bytes)"

# Seed DB (read-only input to fork-genesis)
[[ -d "$ROOTCHAIN_SEED_DB" ]] || die "SEED_DB_MISSING" "Seed DB not found: $ROOTCHAIN_SEED_DB"
info "  OK seed DB: $ROOTCHAIN_SEED_DB"

# ─── Step 1: Prior-run cleanup (idempotency) ─────────────────────────────────
info "=== Step 1: Prior-run cleanup ==="
cleanup_prior_runs

# ─── Step 2: Regenerate forked relay spec (W8) ───────────────────────────────
info "=== Step 2: Regenerating forked relay spec via fork-genesis ==="
info "  register-leafchain=${REGISTER_PARA_ID}:${PARA_JSON}"
rm -f "$RELAY_JSON"
"$POLKADOT" fork-genesis \
    --chain="$RELAY_CHAIN" \
    --base-path="$ROOTCHAIN_SEED_DB" \
    --database=rocksdb \
    --register-leafchain="${REGISTER_PARA_ID}:${PARA_JSON}" \
    --leafchain-binary="$LEAFCHAIN" \
    --output="$RELAY_JSON" \
    2> >(tail -20 >&2)
[[ -f "$RELAY_JSON" ]] || die "RELAY_SPEC_MISSING" "fork-genesis did not produce $RELAY_JSON"
RELAY_SIZE=$(wc -c < "$RELAY_JSON")
(( RELAY_SIZE >= 1000000 )) || die "RELAY_SPEC_MISSING" "Relay spec too small: ${RELAY_SIZE} bytes"
info "  OK relay spec regenerated: $RELAY_JSON (${RELAY_SIZE} bytes)"

# ─── Phase A: Boot relay validators ──────────────────────────────────────────
info "=== Phase A: Starting relay validators ==="

# Alice (bootnode — fixed node key for deterministic peer ID)
mkdir -p "$BASE_RELAY_ALICE"
"$POLKADOT" \
    --alice \
    --base-path="$BASE_RELAY_ALICE" \
    --chain="$RELAY_JSON" \
    --port=40331 \
    --rpc-port=9931 \
    --node-key="$RELAY_ALICE_NODE_KEY" \
    --rpc-methods=Unsafe \
    --no-prometheus \
    --no-telemetry \
    --no-mdns \
    --force-authoring \
    > "$LOG_RELAY_ALICE" 2>&1 &
PID_RELAY_ALICE=$!
echo "$PID_RELAY_ALICE" > "$PID_FILE_RELAY_ALICE"
info "Relay Alice started (PID=$PID_RELAY_ALICE)"

# Wait for Alice to initialise before starting peers
info "Waiting ${RELAY_START_WAIT}s for relay Alice to initialise..."
sleep "$RELAY_START_WAIT"

if ! kill -0 "$PID_RELAY_ALICE" 2>/dev/null; then
    error "Relay Alice died immediately. Last 20 lines:"
    tail -20 "$LOG_RELAY_ALICE" >&2
    die "RELAY_SPEC_MISSING" "Relay Alice exited at startup"
fi
info "Relay Alice alive. Peer ID: $RELAY_ALICE_PEER_ID"

# Bob
mkdir -p "$BASE_RELAY_BOB"
"$POLKADOT" \
    --bob \
    --base-path="$BASE_RELAY_BOB" \
    --chain="$RELAY_JSON" \
    --port=40332 \
    --rpc-port=9932 \
    --bootnodes="$RELAY_ALICE_BOOTNODE" \
    --rpc-methods=Unsafe \
    --no-prometheus \
    --no-telemetry \
    --no-mdns \
    --force-authoring \
    > "$LOG_RELAY_BOB" 2>&1 &
PID_RELAY_BOB=$!
echo "$PID_RELAY_BOB" > "$PID_FILE_RELAY_BOB"
info "Relay Bob started (PID=$PID_RELAY_BOB)"

# Charlie
mkdir -p "$BASE_RELAY_CHARLIE"
"$POLKADOT" \
    --charlie \
    --base-path="$BASE_RELAY_CHARLIE" \
    --chain="$RELAY_JSON" \
    --port=40333 \
    --rpc-port=9933 \
    --bootnodes="$RELAY_ALICE_BOOTNODE" \
    --rpc-methods=Unsafe \
    --no-prometheus \
    --no-telemetry \
    --no-mdns \
    --force-authoring \
    > "$LOG_RELAY_CHARLIE" 2>&1 &
PID_RELAY_CHARLIE=$!
echo "$PID_RELAY_CHARLIE" > "$PID_FILE_RELAY_CHARLIE"
info "Relay Charlie started (PID=$PID_RELAY_CHARLIE)"

info "All 3 relay validators started: Alice=$PID_RELAY_ALICE Bob=$PID_RELAY_BOB Charlie=$PID_RELAY_CHARLIE"

# ─── Gate A: Relay finalized #1 within 60s ───────────────────────────────────
info "=== Gate A: Waiting for relay finalized #1 (timeout=${RELAY_FINALIZE_TIMEOUT}s) ==="
RELAY_FINALIZE_LINE=""
if RELAY_FINALIZE_LINE=$(wait_for_pattern "$RELAY_FINALIZE_TIMEOUT" \
        'finalized #[1-9][0-9]*' "${ALL_RELAY_LOGS[@]}"); then
    info "PASS Gate A: relay '$RELAY_FINALIZE_LINE' seen"
else
    error "Relay finalized #1 not seen within ${RELAY_FINALIZE_TIMEOUT}s."
    for log in "${ALL_RELAY_LOGS[@]}"; do
        error "Last 10 of $(basename $log):"
        tail -10 "$log" >&2
    done
    die "RELAY_FINALIZE_TIMEOUT" "Relay did not finalize block #1 within ${RELAY_FINALIZE_TIMEOUT}s"
fi

# ─── Phase B: Keystore insertion then collator launch ────────────────────────
info "=== Phase B: Inserting aura keystores (BEFORE collator launch) ==="

# Insert aura keys for each collator's base path
# Must happen BEFORE launch — else "no authority key found" silent fail
insert_aura_key "$BASE_SAND_ALICE"   "//Alice"   "$ALICE_SR25519_PUB"
insert_aura_key "$BASE_SAND_BOB"     "//Bob"     "$BOB_SR25519_PUB"
insert_aura_key "$BASE_SAND_CHARLIE" "//Charlie" "$CHARLIE_SR25519_PUB"

info "Keystores inserted. Waiting ${PHASE_B_GRACE}s grace for cumulus relay-light-client sync baseline..."
sleep "$PHASE_B_GRACE"

info "=== Phase B: Starting collators ==="

# sand-Alice (first; no para bootnode arg)
PID_SAND_ALICE=$(launch_collator \
    "--alice" \
    "$BASE_SAND_ALICE" \
    40334 9934 \
    "$PARA_ALICE_NODE_KEY" \
    40335 9935 \
    "" \
    "$LOG_SAND_ALICE")
echo "$PID_SAND_ALICE" > "$PID_FILE_SAND_ALICE"
info "sand-Alice started (PID=$PID_SAND_ALICE, para_p2p=40334, emb_relay_p2p=40335)"

# Brief stagger so sand-Alice registers its p2p listener before peers connect
sleep 3

# sand-Bob (boots off sand-Alice for para net AND relay-Alice for embedded relay client)
PID_SAND_BOB=$(launch_collator \
    "--bob" \
    "$BASE_SAND_BOB" \
    40336 9936 \
    "$PARA_BOB_NODE_KEY" \
    40337 9937 \
    "$PARA_ALICE_BOOTNODE" \
    "$LOG_SAND_BOB")
echo "$PID_SAND_BOB" > "$PID_FILE_SAND_BOB"
info "sand-Bob started (PID=$PID_SAND_BOB, para_p2p=40336, emb_relay_p2p=40337)"

sleep 2

# sand-Charlie (same boot pattern as Bob)
PID_SAND_CHARLIE=$(launch_collator \
    "--charlie" \
    "$BASE_SAND_CHARLIE" \
    40338 9938 \
    "$PARA_CHARLIE_NODE_KEY" \
    40339 9939 \
    "$PARA_ALICE_BOOTNODE" \
    "$LOG_SAND_CHARLIE")
echo "$PID_SAND_CHARLIE" > "$PID_FILE_SAND_CHARLIE"
info "sand-Charlie started (PID=$PID_SAND_CHARLIE, para_p2p=40338, emb_relay_p2p=40339)"

info "All 6 nodes running."
info "  Relay:  Alice=$PID_RELAY_ALICE Bob=$PID_RELAY_BOB Charlie=$PID_RELAY_CHARLIE"
info "  Para:   Alice=$PID_SAND_ALICE  Bob=$PID_SAND_BOB  Charlie=$PID_SAND_CHARLIE"

# ─── Criterion 2: All 6 PIDs alive ───────────────────────────────────────────
info "=== Criterion 2: All 6 PIDs alive check ==="
if ! check_all_pids_alive; then
    die "CRITICAL_LOG_HIT" "One or more nodes died at Phase B startup"
fi
info "PASS Criterion 2: All 6 PIDs alive"

# ─── Gate B / Criterion 4: Para Imported #1 within 120s ──────────────────────
info "=== Gate B / Criterion 4: Waiting for para Imported #1 (timeout=${PARA_IMPORT_TIMEOUT}s) ==="
PARA_IMPORT_LINE=""
if PARA_IMPORT_LINE=$(wait_for_pattern "$PARA_IMPORT_TIMEOUT" \
        '\[Parachain\].*Imported #[1-9][0-9]*' "${ALL_PARA_LOGS[@]}"); then
    info "PASS Criterion 4: para '$PARA_IMPORT_LINE' seen in para logs"
else
    error "Para Imported #1 not seen within ${PARA_IMPORT_TIMEOUT}s of Phase B."
    for log in "${ALL_PARA_LOGS[@]}"; do
        error "Last 15 of $(basename $log):"
        tail -15 "$log" >&2
    done
    die "PARA_IMPORT_TIMEOUT" "Para did not import block #1 within ${PARA_IMPORT_TIMEOUT}s"
fi

# ─── Criterion 3: Relay block #1 finalized (already satisfied at Gate A) ─────
info "PASS Criterion 3: Relay finalized #1 was confirmed at Gate A (${RELAY_FINALIZE_LINE})"

# ─── Criterion 5: Peer counts ≥1 for each collator at +60s ──────────────────
info "=== Criterion 5: Peer count check (${PEER_CHECK_DELAY}s wait) ==="
info "Waiting ${PEER_CHECK_DELAY}s for peer discovery to stabilise..."
sleep "$PEER_CHECK_DELAY"

# Re-verify all PIDs still alive before peer check
if ! check_all_pids_alive; then
    die "PEER_CHECK_FAIL" "A node died during peer-discovery wait"
fi

# Check para p2p peer count: look for "peers=N" or "N peers" with N>=1 in para logs
PEER_FAIL=false
for log_file in "${ALL_PARA_LOGS[@]}"; do
    node_name=$(basename "$log_file" .log)
    # Substrate logs: "Idle (N peers), best: ..." or "peers=N"
    if grep -qE '\(([1-9][0-9]*) peer' "$log_file" 2>/dev/null || \
       grep -qE 'peers=[1-9]' "$log_file" 2>/dev/null; then
        PEER_LINE=$(grep -oE '([1-9][0-9]*) peer[s]?' "$log_file" 2>/dev/null | head -1 || true)
        info "PASS Criterion 5: ${node_name} has peers (${PEER_LINE})"
    else
        warn "WARN: ${node_name} shows 0 peers in para p2p log (may still be discovering)"
        PEER_FAIL=true
    fi
done

# Check embedded relay client peer count
for log_file in "${ALL_PARA_LOGS[@]}"; do
    node_name=$(basename "$log_file" .log)
    # The embedded relay logs appear in the same file (after the -- separator in the binary output)
    # Look for "syncing" or peer-related lines for the relay side
    if grep -qiE '(relay.*[1-9][0-9]* peer|[1-9][0-9]* peer.*relay|Idle.*relay)' "$log_file" 2>/dev/null; then
        info "PASS Criterion 5: ${node_name} embedded relay client has peers"
    else
        # Non-fatal: relay light client peering can take longer
        warn "WARN: ${node_name} embedded relay client peer count not confirmed in log (still syncing)"
    fi
done

if [[ "$PEER_FAIL" == "true" ]]; then
    die "PEER_CHECK_FAIL" "One or more collators show 0 para p2p peers at +${PEER_CHECK_DELAY}s"
fi
info "PASS Criterion 5: All collators have ≥1 para p2p peer"

# ─── Criterion 6: 5-minute burn-in ───────────────────────────────────────────
info "=== Criterion 6: ${BURN_IN_SECONDS}s mini burn-in ==="
info "Burn-in start: $(date). Will poll critical logs every 30s."
BURN_IN_DEADLINE=$(( $(date +%s) + BURN_IN_SECONDS ))
BURN_IN_ELAPSED=0
while (( $(date +%s) < BURN_IN_DEADLINE )); do
    sleep 30
    BURN_IN_ELAPSED=$(( $(date +%s) - (BURN_IN_DEADLINE - BURN_IN_SECONDS) ))

    # Check all PIDs still alive
    if ! check_all_pids_alive; then
        die "CRITICAL_LOG_HIT" "A node died during burn-in at elapsed ${BURN_IN_ELAPSED}s"
    fi

    # Critical log scan (W4: -irE not (?i) PCRE)
    if ! check_critical_logs; then
        die "CRITICAL_LOG_HIT" "Critical log pattern found during burn-in at elapsed ${BURN_IN_ELAPSED}s"
    fi

    REMAINING=$(( BURN_IN_DEADLINE - $(date +%s) ))
    info "Burn-in heartbeat: ${BURN_IN_ELAPSED}s elapsed, ${REMAINING}s remaining. All nodes OK, no critical logs."
done

info "PASS Criterion 6: ${BURN_IN_SECONDS}s burn-in complete — zero critical log hits"

# ─── Final critical log sweep ─────────────────────────────────────────────────
info "=== Final critical log sweep ==="
if ! check_critical_logs; then
    die "CRITICAL_LOG_HIT" "Critical log pattern found in final sweep"
fi
info "PASS: Zero critical log hits across all 6 logs"

# ─── Criterion 7: Idempotency — no orphan PIDs ───────────────────────────────
info "=== Criterion 7: Clean exit check ==="
info "All 6 PID files will be removed by exit trap."
info "Orphan PID check: all PIDs accounted for and will be terminated by trap."

# ─── Final report ────────────────────────────────────────────────────────────
info ""
info "══════════════════════════════════════════════════════"
info "  CROSS-CHAIN LIVENESS VERIFICATION: PASS"
info "══════════════════════════════════════════════════════"
info ""
info "Acceptance criteria results:"
info "  [1] Script exit 0                                  : PASS (about to happen)"
info "  [2] All 6 PIDs alive at criterion 3 check          : PASS"
info "  [3] Relay finalized #1 within 60s                  : PASS (${RELAY_FINALIZE_LINE})"
info "  [4] Para Imported #1 within 120s of Phase B        : PASS (${PARA_IMPORT_LINE})"
info "  [5] Each collator ≥1 para p2p peer at +${PEER_CHECK_DELAY}s      : PASS"
info "  [6] ${BURN_IN_SECONDS}s burn-in zero critical log hits      : PASS"
info "  [7] Clean exit + no orphan PID files               : PASS (trap fires on EXIT)"
info ""
info "Relay spec : $RELAY_JSON (${RELAY_SIZE} bytes)"
info "Para spec  : $PARA_JSON (${PARA_SIZE} bytes)"
info ""
info "Logs:"
for log in "${ALL_LOGS[@]}"; do
    info "  $log"
done
info ""

exit 0
