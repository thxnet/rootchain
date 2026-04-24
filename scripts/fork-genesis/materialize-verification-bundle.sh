#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
materialize-verification-bundle.sh — resolve, validate, and unpack a leafchain verification bundle

Usage:
  scripts/fork-genesis/materialize-verification-bundle.sh \
    --bundle-ref=REF \
    --output-dir=PATH \
    --expected-relay-chain=CHAIN \
    --expected-para-id=N \
    --expected-para-chain-id=CHAIN \
    [--expected-scenario-id=SCENARIO] \
    [--leafchain-binary-ref=REF]

Supported bundle refs:
  - absolute or relative local directory containing manifest.json + payload files
  - OCI image ref containing /bundle/manifest.json etc.

On success the script prints shell assignments to stdout:
  VERIFY_BUNDLE_DIR=...
  VERIFY_BUNDLE_MANIFEST=...
  VERIFY_BUNDLE_PARA_JSON=...
  VERIFY_BUNDLE_VALIDATION_CODE=...
  VERIFY_BUNDLE_GENESIS_STATE=...
  VERIFY_BUNDLE_LEAFCHAIN_BINARY_REF=...
  VERIFY_BUNDLE_LEAFCHAIN_BIN=...
EOF
}

require_arg() {
  local name="$1"
  local value="$2"
  [[ -n "$value" ]] || {
    echo "missing required argument: $name" >&2
    exit 1
  }
}

bundle_ref=""
output_dir=""
expected_scenario_id=""
expected_relay_chain=""
expected_para_id=""
expected_para_chain_id=""
leafchain_binary_ref_override=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --help|-h)
      usage
      exit 0
      ;;
    --bundle-ref=*)
      bundle_ref="${1#*=}"
      ;;
    --output-dir=*)
      output_dir="${1#*=}"
      ;;
    --expected-scenario-id=*)
      expected_scenario_id="${1#*=}"
      ;;
    --expected-relay-chain=*)
      expected_relay_chain="${1#*=}"
      ;;
    --expected-para-id=*)
      expected_para_id="${1#*=}"
      ;;
    --expected-para-chain-id=*)
      expected_para_chain_id="${1#*=}"
      ;;
    --leafchain-binary-ref=*)
      leafchain_binary_ref_override="${1#*=}"
      ;;
    --*)
      echo "unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
    *)
      echo "unexpected positional argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
  shift
done

require_arg --bundle-ref "$bundle_ref"
require_arg --output-dir "$output_dir"
require_arg --expected-relay-chain "$expected_relay_chain"
require_arg --expected-para-id "$expected_para_id"
require_arg --expected-para-chain-id "$expected_para_chain_id"

command -v python3 >/dev/null 2>&1 || {
  echo "python3 is required" >&2
  exit 1
}
command -v sha256sum >/dev/null 2>&1 || {
  echo "sha256sum is required" >&2
  exit 1
}

mkdir -p "$output_dir"
work_dir="$(mktemp -d "$output_dir/.materialize.XXXXXX")"
trap 'rm -rf "$work_dir"' EXIT

source_bundle_dir=""
if [[ -d "$bundle_ref" ]]; then
  source_bundle_dir="$bundle_ref"
else
  command -v docker >/dev/null 2>&1 || {
    echo "docker is required to materialize OCI bundle ref: $bundle_ref" >&2
    exit 1
  }
  docker image inspect "$bundle_ref" >/dev/null 2>&1 || docker pull "$bundle_ref" >/dev/null
  container_name="verification-bundle-$$"
  docker rm -f "$container_name" >/dev/null 2>&1 || true
  docker create --name "$container_name" "$bundle_ref" >/dev/null
  mkdir -p "$work_dir/source"
  docker cp "$container_name:/bundle/." "$work_dir/source"
  docker rm -f "$container_name" >/dev/null
  source_bundle_dir="$work_dir/source"
fi

manifest_source="$source_bundle_dir/manifest.json"
[[ -f "$manifest_source" ]] || {
  echo "bundle manifest missing: $manifest_source" >&2
  exit 1
}

validation_json="$work_dir/validation.json"
python3 - \
  "$manifest_source" \
  "$source_bundle_dir" \
  "$expected_scenario_id" \
  "$expected_relay_chain" \
  "$expected_para_id" \
  "$expected_para_chain_id" \
  "$validation_json" <<'PY'
import hashlib, json, os, sys
manifest_path, bundle_dir, expected_scenario_id, expected_relay_chain, expected_para_id, expected_para_chain_id, out_path = sys.argv[1:]
with open(manifest_path, 'r', encoding='utf-8') as fh:
    manifest = json.load(fh)

def fail(msg):
    print(msg, file=sys.stderr)
    raise SystemExit(1)

if manifest.get('schema_version') != 1:
    fail(f"unsupported schema_version: {manifest.get('schema_version')!r}")
if manifest.get('artifact_kind') != 'leafchain-cross-chain-verification-bundle':
    fail(f"unexpected artifact_kind: {manifest.get('artifact_kind')!r}")
if expected_scenario_id and manifest.get('scenario_id') != expected_scenario_id:
    fail(f"scenario_id mismatch: expected {expected_scenario_id!r}, got {manifest.get('scenario_id')!r}")
if manifest.get('relay_chain') != expected_relay_chain:
    fail(f"relay_chain mismatch: expected {expected_relay_chain!r}, got {manifest.get('relay_chain')!r}")
if int(manifest.get('para_id')) != int(expected_para_id):
    fail(f"para_id mismatch: expected {expected_para_id!r}, got {manifest.get('para_id')!r}")
if manifest.get('para_chain_id') != expected_para_chain_id:
    fail(f"para_chain_id mismatch: expected {expected_para_chain_id!r}, got {manifest.get('para_chain_id')!r}")
rootchain_contract = manifest.get('rootchain_contract') or {}
if rootchain_contract.get('register_leafchain_mode') != 'spec-json':
    fail(f"unsupported register_leafchain_mode: {rootchain_contract.get('register_leafchain_mode')!r}")
paths = manifest.get('paths') or {}
checksums = manifest.get('checksums') or {}
resolved = {}
bundle_root = os.path.abspath(bundle_dir)
for key, checksum_key in [
    ('para_spec', 'para_spec_sha256'),
    ('validation_code', 'validation_code_sha256'),
    ('genesis_state', 'genesis_state_sha256'),
]:
    rel = paths.get(key)
    if not rel:
        fail(f"manifest missing paths.{key}")
    abs_path = os.path.abspath(os.path.join(bundle_root, rel))
    if not (abs_path == bundle_root or abs_path.startswith(bundle_root + os.sep)):
        fail(f"path escapes bundle dir for {key}: {rel!r}")
    if not os.path.isfile(abs_path):
        fail(f"bundle payload missing for {key}: {abs_path}")
    with open(abs_path, 'rb') as fh:
        digest = hashlib.sha256(fh.read()).hexdigest()
    if digest != checksums.get(checksum_key):
        fail(f"checksum mismatch for {key}: expected {checksums.get(checksum_key)!r}, got {digest!r}")
    resolved[key] = abs_path
leafchain_binary = manifest.get('leafchain_binary') or {}
with open(out_path, 'w', encoding='utf-8') as fh:
    json.dump({
        'manifest': manifest,
        'resolved': resolved,
        'leafchain_binary_image': leafchain_binary.get('image') or '',
    }, fh, indent=2, sort_keys=True)
    fh.write('\n')
PY

manifest_path="$output_dir/manifest.json"
para_json_path="$output_dir/para-spec.raw.json"
validation_code_path="$output_dir/validation-code.wasm"
genesis_state_path="$output_dir/genesis-state.bin"
cp "$manifest_source" "$manifest_path"
python3 - "$validation_json" "$para_json_path" "$validation_code_path" "$genesis_state_path" <<'PY'
import json, shutil, sys
validation_json, para_json_path, validation_code_path, genesis_state_path = sys.argv[1:]
with open(validation_json, 'r', encoding='utf-8') as fh:
    validation = json.load(fh)
shutil.copyfile(validation['resolved']['para_spec'], para_json_path)
shutil.copyfile(validation['resolved']['validation_code'], validation_code_path)
shutil.copyfile(validation['resolved']['genesis_state'], genesis_state_path)
PY

leafchain_binary_ref="${leafchain_binary_ref_override}"
if [[ -z "$leafchain_binary_ref" ]]; then
  leafchain_binary_ref="$(python3 - "$validation_json" <<'PY'
import json, sys
with open(sys.argv[1], 'r', encoding='utf-8') as fh:
    print(json.load(fh).get('leafchain_binary_image', ''))
PY
)"
fi

leafchain_bin_path=""
if [[ -n "$leafchain_binary_ref" ]]; then
  command -v docker >/dev/null 2>&1 || {
    echo "docker is required to materialize leafchain binary ref: $leafchain_binary_ref" >&2
    exit 1
  }
  docker image inspect "$leafchain_binary_ref" >/dev/null 2>&1 || docker pull "$leafchain_binary_ref" >/dev/null
  container_name="verification-binary-$$"
  docker rm -f "$container_name" >/dev/null 2>&1 || true
  docker create --name "$container_name" "$leafchain_binary_ref" >/dev/null
  leafchain_bin_path="$output_dir/thxnet-leafchain"
  docker cp "$container_name:/usr/local/bin/thxnet-leafchain" "$leafchain_bin_path"
  docker rm -f "$container_name" >/dev/null
  chmod +x "$leafchain_bin_path"
fi

printf 'VERIFY_BUNDLE_DIR=%q\n' "$output_dir"
printf 'VERIFY_BUNDLE_MANIFEST=%q\n' "$manifest_path"
printf 'VERIFY_BUNDLE_PARA_JSON=%q\n' "$para_json_path"
printf 'VERIFY_BUNDLE_VALIDATION_CODE=%q\n' "$validation_code_path"
printf 'VERIFY_BUNDLE_GENESIS_STATE=%q\n' "$genesis_state_path"
printf 'VERIFY_BUNDLE_LEAFCHAIN_BINARY_REF=%q\n' "$leafchain_binary_ref"
printf 'VERIFY_BUNDLE_LEAFCHAIN_BIN=%q\n' "$leafchain_bin_path"
