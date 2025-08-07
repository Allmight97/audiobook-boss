#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"

echo "== Large files (>400 lines) =="
find "$ROOT_DIR" -type f \
  \( -name "*.rs" -o -name "*.ts" -o -name "*.tsx" -o -name "*.js" \) \
  -not -path "*/target/*" -not -path "*/node_modules/*" \
  -exec wc -l {} + | awk '$1 > 400' | sort -nr

echo
echo "== Functions likely >60 lines (heuristic) =="
rg -n --no-heading --color never --glob '!target/**' --glob '!node_modules/**' \
  -e '^(\s*(pub\s+)?(async\s+)?(fn|function)\s+\w+\s*\([^)]*\)\s*(->[^\{]+)?\{)' "$ROOT_DIR" \
  | while IFS=: read -r file line _; do
      total=$(tail -n +"$line" "$file" | awk 'BEGIN{d=0; n=0} {n++; d+=gsub(/\{/,"&"); d-=gsub(/\}/,"&"); if(n>1 && d==0){print n; exit}}')
      if [ -n "$total" ] && [ "$total" -gt 60 ]; then
        echo "$file:$line (~$total lines)"
      fi
    done | sort -t: -k2,2n


