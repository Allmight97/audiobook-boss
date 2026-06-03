#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

bindings_file="src/lib/generated/tauri.ts"
mode="verify"
use_staged=false
script_started_at="$(date +%s)"

elapsed_since() {
  local started_at="$1"
  echo "$(( $(date +%s) - started_at ))"
}

usage() {
  cat <<'USAGE'
Usage:
  scripts/check-generated-bindings.sh [--mode verify|local|sync] [--staged]

Modes:
  verify  Always regenerate and fail if the generated bindings drift.
  local   Skip regeneration unless contract-related files changed.
  sync    Regenerate and auto-stage generated bindings when drift is found.

Flags:
  --staged  Evaluate change detection from staged files only.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --mode)
      if [[ $# -lt 2 ]]; then
        usage
        exit 1
      fi
      mode="$2"
      shift 2
      ;;
    --staged)
      use_staged=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "[check-generated-bindings] Unknown argument: $1" >&2
      usage
      exit 1
      ;;
  esac
done

case "$mode" in
  verify|local|sync) ;;
  *)
    echo "[check-generated-bindings] Invalid mode: $mode" >&2
    usage
    exit 1
    ;;
esac

collect_changed_files() {
  if $use_staged; then
    git diff --cached --name-only --diff-filter=ACMR || true
    return
  fi

  git diff --name-only HEAD -- || true
  git ls-files --others --exclude-standard || true
}

has_contract_related_changes() {
  local file
  while IFS= read -r file; do
    [[ -z "$file" ]] && continue
    if [[ "$file" == "src-tauri/Cargo.toml" ]]; then
      if src_tauri_cargo_toml_has_contract_diff; then
        return 0
      fi
      continue
    fi
    if [[ "$file" == "crates/abb-"*"-core/Cargo.toml" || "$file" == "crates/abb-"*"-core/"*".rs" ]]; then
      return 0
    fi
    if [[ "$file" == "src-tauri/build.rs" || "$file" == "src-tauri/src/"*".rs" ]]; then
      return 0
    fi
    if [[ "$file" == "$bindings_file" ]]; then
      return 0
    fi
  done < <(collect_changed_files)

  return 1
}

src_tauri_cargo_toml_has_contract_diff() {
  local diff_output changed_lines

  if $use_staged; then
    diff_output="$(git diff --cached -U0 -- src-tauri/Cargo.toml || true)"
  else
    diff_output="$(git diff HEAD -U0 -- src-tauri/Cargo.toml || true)"
  fi

  changed_lines="$(
    printf '%s\n' "$diff_output" |
      awk '/^[+-]/ && ! /^(---|\+\+\+)/ { print }'
  )"

  if [[ -z "$changed_lines" ]]; then
    return 1
  fi

  if printf '%s\n' "$changed_lines" | rg -q -v '^[+-]version = "[^"]+"$'; then
    return 0
  fi

  echo "[check-generated-bindings] Local mode: src-tauri/Cargo.toml changed only package version; skipping regeneration trigger."
  return 1
}

regenerate_and_verify() {
  local before_file
  local before_exists="false"
  local started_at finished_at elapsed
  before_file="$(mktemp)"
  if [[ ! -f "$bindings_file" ]]; then
    echo "[check-generated-bindings] Missing $bindings_file. Generating it now..."
  else
    before_exists="true"
    cp "$bindings_file" "$before_file"
  fi

  echo "[check-generated-bindings] Mode: $mode; staged: $use_staged"
  echo "[check-generated-bindings] Regenerating TypeScript bindings..."
  started_at="$(date +%s)"
  bun run bindings:generate
  finished_at="$(date +%s)"
  elapsed=$((finished_at - started_at))
  echo "[check-generated-bindings] Generation completed in ${elapsed}s."

  if [[ "$before_exists" != "true" ]] || ! cmp -s "$before_file" "$bindings_file"; then
    echo "[check-generated-bindings] Generated bindings are stale."
    echo "[check-generated-bindings] Run: bun run bindings:generate"
    if [[ "$before_exists" == "true" ]]; then
      diff -u "$before_file" "$bindings_file" || true
    else
      git --no-pager diff -- "$bindings_file" || true
    fi
    rm -f "$before_file"
    return 1
  fi
  rm -f "$before_file"

  echo "[check-generated-bindings] Bindings are up to date."
}

if [[ "$mode" == "local" ]] && ! has_contract_related_changes; then
  echo "[check-generated-bindings] Local mode: no contract-related changes detected; skipping regeneration."
  echo "[check-generated-bindings] Completed in $(elapsed_since "$script_started_at")s."
  exit 0
fi

if [[ "$mode" == "sync" ]]; then
  sync_started_at="$(date +%s)"
  if [[ ! -f "$bindings_file" ]]; then
    echo "[check-generated-bindings] Missing $bindings_file. Generating it now..."
  fi
  echo "[check-generated-bindings] Mode: sync; staged: $use_staged"
  echo "[check-generated-bindings] Regenerating TypeScript bindings..."
  bun run bindings:generate
  echo "[check-generated-bindings] Generation completed in $(elapsed_since "$sync_started_at")s."
  if ! git diff --quiet -- "$bindings_file"; then
    git add "$bindings_file"
    echo "[check-generated-bindings] Staged updated $bindings_file"
  fi
  echo "[check-generated-bindings] Binding sync complete in $(elapsed_since "$script_started_at")s."
  exit 0
fi

regenerate_and_verify
echo "[check-generated-bindings] Completed in $(elapsed_since "$script_started_at")s."
