#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

mode="audit"
name=""
keep_days=7
include_diffs=0

usage() {
  cat <<'USAGE'
Usage:
  scripts/repomix-handoff.sh [options]

Options:
  --mode <audit|full>                 Output profile (default: audit)
  --name <artifact-name>              Output basename without extension
  --keep-days <days>                  Delete local handoff XML files older than N days (default: 7)
  --include-diffs                     Include current git diff in artifact
  -h, --help                          Show help

Examples:
  scripts/repomix-handoff.sh --mode audit
  scripts/repomix-handoff.sh --mode full --name sprint-42-context
  scripts/repomix-handoff.sh --mode audit --include-diffs
USAGE
}

log() {
  echo "[repomix-handoff] $*"
}

fail() {
  echo "[repomix-handoff] $*" >&2
  exit 1
}

require() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "Required tool '$1' not found."
  fi
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --mode)
      mode="${2:-}"
      shift 2
      ;;
    --name)
      name="${2:-}"
      shift 2
      ;;
    --keep-days)
      keep_days="${2:-}"
      shift 2
      ;;
    --include-diffs)
      include_diffs=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      fail "Unknown argument: $1"
      ;;
  esac
done

case "$mode" in
  audit|full) ;;
  *)
    fail "Invalid mode '$mode'. Use audit or full."
    ;;
esac

if ! [[ "$keep_days" =~ ^[0-9]+$ ]]; then
  fail "--keep-days must be a non-negative integer."
fi

if [[ -z "$name" ]]; then
  name="${mode}-$(date +%Y%m%d-%H%M%S)"
fi

require bunx

output_dir=".repomix/handoffs"
mkdir -p "$output_dir"
output_path="$output_dir/${name}.xml"
deleted_count="$(find "$output_dir" -type f -name '*.xml' -mtime +"$keep_days" -print -delete | wc -l | tr -d ' ')"
log "Cleanup: removed ${deleted_count} stale artifact(s) older than ${keep_days} day(s)."

repomix_args=()
repomix_args+=(.)

repomix_args+=(
  --output "$output_path"
  --style xml
)

if [[ "$include_diffs" -eq 1 ]]; then
  repomix_args+=(--include-diffs)
fi

case "$mode" in
  audit)
    repomix_args+=(
      --compress
      --remove-comments
      --remove-empty-lines
      --include "**/AGENTS.md,README.md,package.json,Cargo.toml,src/**/*.{ts,js,svelte},src-tauri/src/**/*.{rs,toml},src-tauri/Cargo.toml,docs/decisions/**/*.md,docs/external-apis/**/*.md,scripts/repomix-handoff.sh,scripts/checks.sh"
      --ignore ".agents/**,docs/engineering/**,docs/specs/**,**/__tests__/**,**/*.test.ts,src-tauri/tests/**,node_modules/**,target/**,dist/**,dist-ssr/**,coverage/**,site/**,media/**,.git/**,**/*.lock,**/*.{svg,png,jpg,jpeg,gif,webp,ico,pdf,mp3,m4b,wav,flac,zip}"
      --token-count-tree 150
      --top-files-len 20
    )
    ;;
  full)
    repomix_args+=(
      --compress
      --include "**/AGENTS.md,README.md,package.json,Cargo.toml,src/**/*.{ts,js,svelte},src-tauri/**/*.{rs,toml},scripts/**/*.{sh,mjs,js,ts,py},docs/**/*.md"
      --ignore ".agents/**,node_modules/**,target/**,dist/**,dist-ssr/**,coverage/**,site/**,media/**,.git/**,**/*.lock,**/*.{svg,png,jpg,jpeg,gif,webp,ico,pdf,mp3,m4b,wav,flac,zip}"
      --include-logs
      --include-logs-count 30
      --token-count-tree 150
      --top-files-len 20
    )
    ;;
esac

log "Mode: ${mode}"
log "Target: local repo"
log "Output: ${output_path}"

bunx repomix "${repomix_args[@]}"

log "Done. Artifact ready at ${output_path}"
