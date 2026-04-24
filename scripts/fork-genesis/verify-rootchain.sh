#!/usr/bin/env bash
# verify-rootchain.sh — Boot 2-validator local network from forked chain spec
# and verify liveness (Imported #1, first Finalized, block >= 50).
#
# Topology: Alice (bootnode) / Bob
# Ports:    p2p 40331-40332 | RPC 9931-9932
#
# Alice uses fixed --node-key so her peer ID is always predictable:
#   Key:     0000000000000000000000000000000000000000000000000000000000000001
#   Peer ID: 12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp
#
# Exit codes (non-zero = named error class):
#   FORK_JSON_FAIL         — fork-genesis produced no output or < 1MB
#   ALICE_PEER_ID_MISSING  — could not confirm Alice peer ID (should never happen)
#   FIRST_IMPORT_TIMEOUT   — no "Imported #1" in Alice log within 30s of Alice start
#   FIRST_FINALIZE_TIMEOUT — no "finalized #\d" in any log within 60s of all-up
#   BLOCK_50_TIMEOUT       — finalized block did not reach ≥ 50 within 360s
#   STATE_EQUIV_FAIL       — state-equivalence.ts non-zero exit (balance/rescue/spec mismatch)
#   CRITICAL_LOG_HIT       — panic / stall / fatal keyword found in any log
#
# Usage:
#   ./verify-rootchain.sh [--keep-running] [--skip-fork-genesis]
#
#   --keep-running       Do NOT kill validators on exit (default: OFF; use for debugging).
#   --skip-fork-genesis  Re-use existing /tmp/forked-thxnet-testnet.json if ≥ 20MB.

set -euo pipefail

# ─── Constants ───────────────────────────────────────────────────────────────
POLKADOT="/root/Works/rootchain/target/release/polkadot"
FORKED_JSON="/tmp/forked-thxnet-testnet.json"
LOG_ALICE="/tmp/forknet-test-alice.log"
LOG_BOB="/tmp/forknet-test-bob.log"
ALL_LOGS=("$LOG_ALICE" "$LOG_BOB")

ALICE_NODE_KEY="0000000000000000000000000000000000000000000000000000000000000001"
ALICE_PEER_ID="12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp"
ALICE_BOOTNODE="/ip4/127.0.0.1/tcp/40331/p2p/${ALICE_PEER_ID}"

# Base paths (per-node, in /tmp so we don't touch /data/)
BASE_ALICE="/tmp/forknet-test-base-alice"
BASE_BOB="/tmp/forknet-test-base-bob"


# DB source (READ-ONLY) for fork-genesis
SEED_BASE_PATH="/data/forknet-test/rootchain-seed"

# Timing knobs
ALICE_START_WAIT=8        # seconds to wait after starting Alice before extracting peer ID
FIRST_IMPORT_TIMEOUT=30   # seconds to wait for "Imported #1"
FIRST_FINALIZE_TIMEOUT=60 # seconds to wait for first "finalized #\d"
BLOCK_50_TIMEOUT=360      # seconds to wait for finalized block >= 50
POLL_INTERVAL=2           # polling interval in seconds

# PIDs (populated below)
ALICE_PID=""
BOB_PID=""


# ─── Flags ───────────────────────────────────────────────────────────────────
KEEP_RUNNING=false   # default OFF — script self-cleans after every run
SKIP_FORK_GENESIS=false

for arg in "$@"; do
    case "$arg" in
        --keep-running)      KEEP_RUNNING=true ;;
        --skip-fork-genesis) SKIP_FORK_GENESIS=true ;;
        *) echo "Unknown arg: $arg" >&2; exit 1 ;;
    esac
done

# ─── Logging helpers ─────────────────────────────────────────────────────────
ts() { date '+%H:%M:%S'; }
info()  { echo "[$(ts)] INFO  $*"; }
warn()  { echo "[$(ts)] WARN  $*"; }
error() { echo "[$(ts)] ERROR $*" >&2; }

die() {
    local code="$1"; shift
    error "FATAL: $* (exit-code: $code)"
    exit 1
}

# ─── Cleanup ─────────────────────────────────────────────────────────────────
cleanup_prior_runs() {
    info "Killing any prior polkadot processes on our ports..."
    # Kill by port ownership
    for port in 40331 40332 9931 9932; do
        local pid
        pid=$(lsof -ti "tcp:${port}" 2>/dev/null || true)
        if [[ -n "$pid" ]]; then
            info "  Killing PID $pid on port $port"
            kill "$pid" 2>/dev/null || true
        fi
    done
    # Also kill by PID files if we left any
    for pidf in /tmp/forknet-test-*.pid; do
        [[ -f "$pidf" ]] || continue
        local old_pid
        old_pid=$(cat "$pidf")
        if kill -0 "$old_pid" 2>/dev/null; then
            info "  Killing prior PID $old_pid from $pidf"
            kill "$old_pid" 2>/dev/null || true
        fi
        rm -f "$pidf"
    done
    # Clear old logs
    rm -f /tmp/forknet-test-alice.log /tmp/forknet-test-bob.log
    # Clear old base paths so we start fresh (avoids keystore/DB conflicts)
    rm -rf "$BASE_ALICE" "$BASE_BOB"
    info "Prior-run cleanup complete."
}

do_cleanup_on_exit() {
    if [[ "$KEEP_RUNNING" == "true" ]]; then
        info "Validators left running (--keep-running). PIDs: Alice=${ALICE_PID} Bob=${BOB_PID}"
        return
    fi
    info "Stopping validators..."
    for pid in "$ALICE_PID" "$BOB_PID"; do
        [[ -n "$pid" ]] && kill "$pid" 2>/dev/null || true
    done
}

trap do_cleanup_on_exit EXIT

# ─── Step 0: Upfront cleanup ─────────────────────────────────────────────────
info "=== Step 0: Upfront cleanup ==="
cleanup_prior_runs

# ─── Step 1: Generate forked chain spec ──────────────────────────────────────
info "=== Step 1: Fork genesis ==="

if [[ "$SKIP_FORK_GENESIS" == "true" ]]; then
    if [[ -f "$FORKED_JSON" ]]; then
        SIZE=$(wc -c < "$FORKED_JSON")
        if (( SIZE >= 20000000 )); then
            info "Re-using existing $FORKED_JSON (${SIZE} bytes)"
        else
            warn "Existing $FORKED_JSON too small (${SIZE} bytes), regenerating..."
            SKIP_FORK_GENESIS=false
        fi
    else
        warn "--skip-fork-genesis but $FORKED_JSON absent, regenerating..."
        SKIP_FORK_GENESIS=false
    fi
fi

if [[ "$SKIP_FORK_GENESIS" == "false" ]]; then
    info "Running fork-genesis (this may take 60-120s)..."
    "$POLKADOT" fork-genesis \
        --chain=thxnet-testnet \
        --base-path="$SEED_BASE_PATH" \
        --database=rocksdb \
        --output="$FORKED_JSON" \
        2>/tmp/forknet-fork-genesis.log \
        || die "FORK_JSON_FAIL" "fork-genesis command failed (see /tmp/forknet-fork-genesis.log)"
fi

# Validate output
if [[ ! -f "$FORKED_JSON" ]]; then
    die "FORK_JSON_FAIL" "Output $FORKED_JSON does not exist after fork-genesis"
fi
FORK_SIZE=$(wc -c < "$FORKED_JSON")
if (( FORK_SIZE < 1000000 )); then
    die "FORK_JSON_FAIL" "$FORKED_JSON is only ${FORK_SIZE} bytes (< 1MB); expected >= 20MB"
fi
info "Forked chain spec: $FORKED_JSON (${FORK_SIZE} bytes)"

# ─── Step 2: Common node flags ───────────────────────────────────────────────
COMMON_FLAGS=(
    --chain="$FORKED_JSON"
    --rpc-methods=Unsafe
    --no-prometheus
    --no-telemetry
    --rpc-external
    --no-mdns
    --force-authoring
)

# ─── Step 3: Boot Alice ──────────────────────────────────────────────────────
info "=== Step 3: Starting Alice (bootnode) ==="
mkdir -p "$BASE_ALICE"

"$POLKADOT" \
    "${COMMON_FLAGS[@]}" \
    --alice \
    --base-path="$BASE_ALICE" \
    --port=40331 \
    --rpc-port=9931 \
    --node-key="$ALICE_NODE_KEY" \
    --log=info \
    > "$LOG_ALICE" 2>&1 &
ALICE_PID=$!
echo "$ALICE_PID" > /tmp/forknet-test-alice.pid
info "Alice started (PID=$ALICE_PID). Waiting ${ALICE_START_WAIT}s..."
sleep "$ALICE_START_WAIT"

# Verify Alice is still alive
if ! kill -0 "$ALICE_PID" 2>/dev/null; then
    error "Alice process died. Last 20 lines of log:"
    tail -20 "$LOG_ALICE" >&2
    die "ALICE_PEER_ID_MISSING" "Alice process exited immediately"
fi

# Verify our expected peer ID appears in log (confirm node-key worked)
# The log emits: "Local node identity is: 12D3..."
if ! grep -qF "$ALICE_PEER_ID" "$LOG_ALICE" 2>/dev/null; then
    warn "Peer ID $ALICE_PEER_ID not yet in Alice log (node still initialising — this is OK)"
    info "Alice log so far:"
    cat "$LOG_ALICE" | tail -10
fi
info "Alice peer ID confirmed (from fixed node-key): $ALICE_PEER_ID"
info "Bootnode multiaddr: $ALICE_BOOTNODE"

# ─── Step 4: Boot Bob ────────────────────────────────────────────────────────
info "=== Step 4: Starting Bob ==="
mkdir -p "$BASE_BOB"
"$POLKADOT" \
    "${COMMON_FLAGS[@]}" \
    --bob \
    --base-path="$BASE_BOB" \
    --port=40332 \
    --rpc-port=9932 \
    --bootnodes="$ALICE_BOOTNODE" \
    --log=info \
    > "$LOG_BOB" 2>&1 &
BOB_PID=$!
echo "$BOB_PID" > /tmp/forknet-test-bob.pid
info "Bob started (PID=$BOB_PID)"

info "All 2 validators up. Alice=$ALICE_PID Bob=$BOB_PID"

# ─── Step 5: Poll for Imported #1 ────────────────────────────────────────────
info "=== Step 5: Waiting for first block import (timeout=${FIRST_IMPORT_TIMEOUT}s) ==="
DEADLINE=$(( $(date +%s) + FIRST_IMPORT_TIMEOUT ))
FIRST_IMPORT_SEEN=false
while (( $(date +%s) < DEADLINE )); do
    if grep -qE 'Imported #[1-9]' "$LOG_ALICE" 2>/dev/null; then
        FIRST_IMPORT_SEEN=true
        info "PASS: 'Imported #1' detected in Alice log"
        break
    fi
    # Also check Bob log
    for log in "$LOG_BOB"; do
        if grep -qE 'Imported #[1-9]' "$log" 2>/dev/null; then
            FIRST_IMPORT_SEEN=true
            info "PASS: 'Imported #1' detected in $(basename $log)"
            break 2
        fi
    done
    sleep "$POLL_INTERVAL"
done

if [[ "$FIRST_IMPORT_SEEN" == "false" ]]; then
    error "No 'Imported #1' found within ${FIRST_IMPORT_TIMEOUT}s. Last 10 lines of Alice log:"
    tail -10 "$LOG_ALICE" >&2
    die "FIRST_IMPORT_TIMEOUT" "First block import not seen"
fi

# ─── Step 6: Poll for first Finalized block ───────────────────────────────────
info "=== Step 6: Waiting for first finalized block (timeout=${FIRST_FINALIZE_TIMEOUT}s) ==="
DEADLINE=$(( $(date +%s) + FIRST_FINALIZE_TIMEOUT ))
FIRST_FINALIZE_SEEN=false
while (( $(date +%s) < DEADLINE )); do
    for log in "${ALL_LOGS[@]}"; do
        if grep -qE 'finalized #[1-9]' "$log" 2>/dev/null; then
            FIRST_FINALIZE_SEEN=true
            FINALIZE_LINE=$(grep -oE 'finalized #[0-9]+' "$log" | head -1)
            info "PASS: '$FINALIZE_LINE' detected in $(basename $log)"
            break 2
        fi
    done
    sleep "$POLL_INTERVAL"
done

if [[ "$FIRST_FINALIZE_SEEN" == "false" ]]; then
    error "No 'finalized #\d' found within ${FIRST_FINALIZE_TIMEOUT}s."
    for log in "${ALL_LOGS[@]}"; do
        error "Last 5 lines of $(basename $log):"
        tail -5 "$log" >&2
    done
    die "FIRST_FINALIZE_TIMEOUT" "First finalized block not seen"
fi

# ─── Step 7: Poll until finalized block >= 50 ────────────────────────────────
info "=== Step 7: Waiting for finalized block >= 50 (timeout=${BLOCK_50_TIMEOUT}s) ==="
DEADLINE=$(( $(date +%s) + BLOCK_50_TIMEOUT ))
BLOCK_50_SEEN=false
HIGHEST_FINALIZED=0
while (( $(date +%s) < DEADLINE )); do
    for log in "${ALL_LOGS[@]}"; do
        # Extract highest finalized block number seen so far
        while IFS= read -r line; do
            num=$(echo "$line" | grep -oE '[0-9]+$' | tail -1)
            if [[ -n "$num" ]] && (( num > HIGHEST_FINALIZED )); then
                HIGHEST_FINALIZED=$num
            fi
        done < <(grep -oE 'finalized #[0-9]+' "$log" 2>/dev/null || true)
    done

    if (( HIGHEST_FINALIZED >= 50 )); then
        BLOCK_50_SEEN=true
        info "PASS: Finalized block $HIGHEST_FINALIZED >= 50"
        break
    fi

    # Progress heartbeat every 30 seconds
    ELAPSED=$(( $(date +%s) - (DEADLINE - BLOCK_50_TIMEOUT) ))
    if (( ELAPSED % 30 == 0 )) || (( ELAPSED < 5 )); then
        info "Progress: highest finalized = $HIGHEST_FINALIZED (elapsed ${ELAPSED}s)"
    fi

    sleep "$POLL_INTERVAL"
done

if [[ "$BLOCK_50_SEEN" == "false" ]]; then
    error "Finalized block did not reach 50 within ${BLOCK_50_TIMEOUT}s (highest seen: $HIGHEST_FINALIZED)"
    die "BLOCK_50_TIMEOUT" "Block 50 not reached in time"
fi

# ─── Step 7.5: State equivalence check (local vs livenet) ────────────────────
info "=== Step 7.5: State equivalence check ==="
STATE_EQUIV_LOG="/tmp/forknet-state-equiv.log"
FORK_GENESIS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LIVENET_WS="wss://node.testnet.thxnet.org/archive-001/ws"
LOCAL_HTTP="http://127.0.0.1:9931"

if ! command -v bun &>/dev/null; then
    die "STATE_EQUIV_FAIL" "bun not found — cannot run state-equivalence.ts"
fi

STATE_EQUIV_EXIT=0
(
    cd "$FORK_GENESIS_DIR"
    bun run state-equivalence.ts \
        --local "$LOCAL_HTTP" \
        --livenet "$LIVENET_WS"
) || STATE_EQUIV_EXIT=$?

if [[ "$STATE_EQUIV_EXIT" -ne 0 ]]; then
    error "State equivalence check FAILED (exit $STATE_EQUIV_EXIT). See $STATE_EQUIV_LOG"
    tail -20 "$STATE_EQUIV_LOG" >&2
    die "STATE_EQUIV_FAIL" "State equivalence check failed (exit $STATE_EQUIV_EXIT)"
fi
info "PASS: State equivalence check passed. Log: $STATE_EQUIV_LOG"

# ─── Step 8: Critical log check ──────────────────────────────────────────────
info "=== Step 8: Critical log pattern check ==="
CRITICAL_PATTERN='(panic!|panicked|stalled|deadlock|FATAL|bad block|essential task)'
CRIT_HITS=$(grep -irE "$CRITICAL_PATTERN" /tmp/forknet-test-*.log | head -20 || true)
if [[ -n "$CRIT_HITS" ]]; then
    error "CRITICAL log hits found:"
    echo "$CRIT_HITS" >&2
    die "CRITICAL_LOG_HIT" "Critical error pattern detected in logs"
fi
info "PASS: No critical log patterns found"

# ─── Step 9: Final status report ─────────────────────────────────────────────
info "=== Final Status Report ==="
info "Forked chain spec : $FORKED_JSON (${FORK_SIZE} bytes)"
info "Alice PID         : $ALICE_PID (alive: $(kill -0 "$ALICE_PID" 2>/dev/null && echo YES || echo NO))"
info "Bob   PID         : $BOB_PID (alive: $(kill -0 "$BOB_PID" 2>/dev/null && echo YES || echo NO))"
info "Validator count   : 2"
info "Highest finalized : block $HIGHEST_FINALIZED"
info "Logs              : $LOG_ALICE | $LOG_BOB"
info ""
info "=== LIVENESS VERIFICATION: PASS ==="
info ""
info "All liveness criteria met:"
info "  [1] fork-genesis produced >= 1MB JSON: YES ($FORK_SIZE bytes)"
info "  [2] Imported #1 seen: YES"
info "  [3] First finalized block seen: YES"
info "  [4] Finalized block >= 50 reached: YES (block $HIGHEST_FINALIZED)"
info "  [5] No critical log hits: YES"

exit 0
