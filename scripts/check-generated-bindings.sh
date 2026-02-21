#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

bindings_file="src/lib/generated/tauri.ts"
mode="verify"
use_staged=false

usage() {
  cat <<'USAGE'
Usage:
  scripts/check-generated-bindings.sh [--mode verify|local|sync] [--staged]

Modes:
  verify  Always regenerate and fail if the generated bindings drift.
  local   Skip regeneration unless contract-related files changed.
  sync    Regenerate and auto-stage generated bindings when drift is found.

Flags:
  --staged  Evaluate change detection from staged files only (for hooks).
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
      return 0
    fi
    if [[ "$file" == "src-tauri/src/"* ]]; then
      return 0
    fi
    if [[ "$file" == "$bindings_file" ]]; then
      return 0
    fi
  done < <(collect_changed_files)

  return 1
}

regenerate_and_verify() {
  if [[ ! -f "$bindings_file" ]]; then
    echo "[check-generated-bindings] Missing $bindings_file. Generating it now..."
  fi

  echo "[check-generated-bindings] Regenerating TypeScript bindings"
  bun run bindings:generate >/dev/null

  if ! git diff --quiet -- "$bindings_file"; then
    echo "[check-generated-bindings] Generated bindings are stale."
    echo "[check-generated-bindings] Run: bun run bindings:generate"
    git --no-pager diff -- "$bindings_file"
    return 1
  fi

  echo "[check-generated-bindings] Bindings are up to date."
}

if [[ "$mode" == "local" ]] && ! has_contract_related_changes; then
  echo "[check-generated-bindings] Local mode: no contract-related changes detected; skipping regeneration."
  exit 0
fi

if [[ "$mode" == "sync" ]]; then
  if [[ ! -f "$bindings_file" ]]; then
    echo "[check-generated-bindings] Missing $bindings_file. Generating it now..."
  fi
  echo "[check-generated-bindings] Regenerating TypeScript bindings (sync mode)"
  bun run bindings:generate >/dev/null
  if ! git diff --quiet -- "$bindings_file"; then
    git add "$bindings_file"
    echo "[check-generated-bindings] Staged updated $bindings_file"
  fi
  echo "[check-generated-bindings] Binding sync complete."
  exit 0
fi

regenerate_and_verify
