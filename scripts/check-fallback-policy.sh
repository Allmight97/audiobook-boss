#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

register_path="docs/fallbacks.md"

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

is_calendar_date() {
  local value="$1"
  local year month day max_day

  if ! [[ "$value" =~ ^([0-9]{4})-([0-9]{2})-([0-9]{2})$ ]]; then
    return 1
  fi

  year="${BASH_REMATCH[1]}"
  month="${BASH_REMATCH[2]}"
  day="${BASH_REMATCH[3]}"

  case "$month" in
    01|03|05|07|08|10|12) max_day=31 ;;
    04|06|09|11) max_day=30 ;;
    02)
      if (( (10#$year % 4 == 0 && 10#$year % 100 != 0) || (10#$year % 400 == 0) )); then
        max_day=29
      else
        max_day=28
      fi
      ;;
    *)
      return 1
      ;;
  esac

  (( 10#$day >= 1 && 10#$day <= max_day ))
}

trim() {
  local value="$1"
  value="${value#"${value%%[![:space:]]*}"}"
  value="${value%"${value##*[![:space:]]}"}"
  printf '%s' "$value"
}

today="${ABB_TODAY:-$(date -u +%F)}"
if ! is_calendar_date "$today"; then
  echo "[fallback-policy] Invalid ABB_TODAY value '$today'; expected a valid YYYY-MM-DD date." >&2
  exit 1
fi

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
expired=0
malformed_renewal=0

while IFS= read -r entry; do
  file="${entry%%:*}"
  rest="${entry#*:}"
  line="${rest%%:*}"
  marker_id="$(echo "$entry" | rg -o "FB-[0-9]{3}")"
  register_line="$(rg -n "^[[:space:]]*\\|[[:space:]]*${marker_id}[[:space:]]*\\|" "$register_path" | head -n 1 || true)"

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
  else
    marker_sunset="$(printf '%s\n' "$chunk" | sed -n 's/.*sunset=\([^[:space:]]*\).*/\1/p' | head -n 1)"
    if ! is_calendar_date "$marker_sunset"; then
      echo "[fallback-policy] Marker ${marker_id} in ${file}:${line} has malformed sunset '${marker_sunset}'."
      missing_meta=1
    fi
  fi

  if [[ -z "$register_line" ]]; then
    continue
  fi

  register_row="${register_line#*:}"
  IFS='|' read -r _ register_marker _ _ _ register_sunset _ register_audit_status _ <<<"$register_row"
  register_marker="$(trim "$register_marker")"
  register_sunset="$(trim "$register_sunset")"
  register_audit_status="$(trim "$register_audit_status")"

  if [[ "$register_marker" != "$marker_id" ]]; then
    echo "[fallback-policy] Register row for ${marker_id} is malformed."
    missing_meta=1
    continue
  fi

  if ! is_calendar_date "$register_sunset"; then
    echo "[fallback-policy] Register entry ${marker_id} has malformed sunset '${register_sunset}'."
    missing_meta=1
    continue
  fi

  renewal_date=""
  renewal_valid=0
  renewal_invalid=0
  if [[ "$register_audit_status" == *"renewal="* ]]; then
    renewal_pattern='renewal=([0-9]{4}-[0-9]{2}-[0-9]{2});[[:space:]]*reason=([^[:space:]].*)'
    if [[ "$register_audit_status" =~ $renewal_pattern ]]; then
      renewal_date="${BASH_REMATCH[1]}"
      renewal_reason="$(trim "${BASH_REMATCH[2]}")"

      if ! is_calendar_date "$renewal_date"; then
        echo "[fallback-policy] Register entry ${marker_id} has malformed renewal date '${renewal_date}'."
        malformed_renewal=1
        renewal_invalid=1
      elif [[ "$renewal_date" < "$register_sunset" || "$renewal_date" == "$register_sunset" ]]; then
        echo "[fallback-policy] Register entry ${marker_id} renewal date '${renewal_date}' does not extend sunset '${register_sunset}'."
        malformed_renewal=1
        renewal_invalid=1
      fi

      if [[ -z "$renewal_reason" ]]; then
        echo "[fallback-policy] Register entry ${marker_id} renewal is missing a reason."
        malformed_renewal=1
        renewal_invalid=1
      fi

      if [[ "$renewal_invalid" -eq 0 ]]; then
        renewal_valid=1
      fi
    else
      echo "[fallback-policy] Register entry ${marker_id} has malformed renewal metadata: ${register_audit_status}"
      malformed_renewal=1
    fi
  fi

  effective_deadline="$register_sunset"
  if [[ "$renewal_valid" -eq 1 ]]; then
    effective_deadline="$renewal_date"
  fi

  if [[ "$today" > "$effective_deadline" ]]; then
    echo "[fallback-policy] Register entry ${marker_id} expired on ${effective_deadline} (today=${today})."
    expired=1
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

  if [[ "$file" == "scripts/check-fallback-policy.sh" ]]; then
    continue
  fi

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

if [[ "$missing_register" -ne 0 || "$missing_meta" -ne 0 || "$missing_code" -ne 0 || "$keyword_fail" -ne 0 || "$expired" -ne 0 || "$malformed_renewal" -ne 0 ]]; then
  echo "[fallback-policy] FAILED"
  exit 1
fi

echo "[fallback-policy] OK"
