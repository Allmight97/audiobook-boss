#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

register_path="docs/engineering/fallback-register.md"

if [[ ! -f "$register_path" ]]; then
  echo "[fallback-policy] Missing $register_path" >&2
  exit 1
fi

require() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "[fallback-policy] Required tool '$1' not found." >&2
    exit 1
  fi
}

require rg
require awk
require sed
require git

tmp_markers="$(mktemp)"
tmp_register="$(mktemp)"
trap 'rm -f "$tmp_markers" "$tmp_register"' EXIT

marker_pattern="^[[:space:]]*(///|//|#|\\*|/\\*)[[:space:]]*FALLBACK\\[FB-[0-9]{3}\\]"
rg -n "$marker_pattern" src src-tauri scripts >"$tmp_markers" || true
rg -o "FB-[0-9]{3}" "$register_path" | sort -u >"$tmp_register"

if [[ ! -s "$tmp_markers" ]]; then
  echo "[fallback-policy] No FALLBACK markers found in src/src-tauri/scripts." >&2
  exit 1
fi

missing_register=0
missing_meta=0

while IFS= read -r entry; do
  file="${entry%%:*}"
  rest="${entry#*:}"
  line="${rest%%:*}"
  marker_id="$(echo "$entry" | rg -o "FB-[0-9]{3}")"

  if ! rg -q "^${marker_id}$" "$tmp_register"; then
    echo "[fallback-policy] Marker ${marker_id} in ${file}:${line} is missing from fallback register."
    missing_register=1
  fi

  start=$((line))
  end=$((line + 4))
  chunk="$(sed -n "${start},${end}p" "$file")"

  if ! echo "$chunk" | rg -q "issue=#[0-9]+"; then
    echo "[fallback-policy] Marker ${marker_id} in ${file}:${line} missing issue=#... metadata."
    missing_meta=1
  fi
  if ! echo "$chunk" | rg -q "sunset="; then
    echo "[fallback-policy] Marker ${marker_id} in ${file}:${line} missing sunset=... metadata."
    missing_meta=1
  fi
done <"$tmp_markers"

missing_code=0
while IFS= read -r id; do
  if ! rg -q "^[[:space:]]*(///|//|#|\\*|/\\*)[[:space:]]*FALLBACK\\[$id\\]" src src-tauri scripts; then
    echo "[fallback-policy] Register entry $id has no matching code marker."
    missing_code=1
  fi
done <"$tmp_register"

collect_changed_files() {
  {
    git diff --name-only
    git diff --cached --name-only
  } | awk 'NF' | sort -u
}

keyword_fail=0
while IFS= read -r file; do
  [[ -z "$file" ]] && continue
  [[ ! -f "$file" ]] && continue

  case "$file" in
    src/*|src-tauri/*|scripts/*) ;;
    *) continue ;;
  esac

  case "$file" in
    *.ts|*.rs|*.js|*.mjs|*.sh) ;;
    *) continue ;;
  esac

  while IFS=: read -r line_num line_text; do
    [[ -z "$line_num" ]] && continue
    if echo "$line_text" | rg -q "FALLBACK\\[FB-[0-9]{3}\\]"; then
      continue
    fi

    if ! rg -q "$marker_pattern" "$file"; then
      echo "[fallback-policy] Unannotated fallback keyword in ${file}:${line_num}: ${line_text}"
      keyword_fail=1
    fi
  done < <(rg -n -i "falling back|fall back|back-compat|backward compatibility|legacy compatibility|retained for compatibility|shim" "$file" || true)
done < <(collect_changed_files)

if [[ "$missing_register" -ne 0 || "$missing_meta" -ne 0 || "$missing_code" -ne 0 || "$keyword_fail" -ne 0 ]]; then
  echo "[fallback-policy] FAILED"
  exit 1
fi

echo "[fallback-policy] OK"
