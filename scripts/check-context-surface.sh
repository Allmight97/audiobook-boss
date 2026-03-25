#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

active_files=(
  "README.md"
  "PLANS.md"
  "docs/fallbacks.md"
  "docs/api-map.md"
  "hooks.json"
)

retained_skills=(
  "audiobook-metadata"
  "contract-guardrails"
  "job-registry-and-progress"
  "lib-research"
  "path-security-validation"
  "tauri-command-conventions"
)

retained_hook_files=(
  ".agents/hooks/common.py"
  ".agents/hooks/session_start.py"
  ".agents/hooks/stop_context_surface.py"
  ".agents/hooks/stop_verification_lane.py"
  ".agents/hooks/stop_ipc_guard.py"
)

removed_skills=(
  "commit"
  "github-issue"
  "land"
  "mp4ameta-patterns"
  "perf-quality-orchestrator"
  "pr-open"
  "pull"
  "push"
  "release-changelog"
  "repomix-explorer"
  "task-tracker"
)

stale_doc_pattern='docs/README\.md|docs/AGENTS\.md|docs/agent-execution\.md|docs/browser-harness\.md|docs/skills-audit\.md|docs/specs/technical-reference\.md|docs/verification\.md|docs/workloop\.md'
stale_check_pattern='check-docs-routing|check-skills-routing'
stale_tracking_pattern='pebbles|task-tracker|\.pebbles|pb ready|pb list|pb show|pb update|pb close|~/.local/bin/pb'

[[ ! -e ".pebbles" ]] || {
  echo "[context-surface] Removed Pebbles directory still present" >&2
  exit 1
}

[[ ! -e ".repomix" ]] || {
  echo "[context-surface] Removed Repomix directory still present" >&2
  exit 1
}

for file in "${active_files[@]}"; do
  [[ -f "$file" ]] || {
    echo "[context-surface] Missing active file: $file" >&2
    exit 1
  }
done

find . -name 'AGENTS.md' -type f ! -path './.git/*' | grep -q . || {
  echo "[context-surface] Missing AGENTS.md files" >&2
  exit 1
}

for skill in "${retained_skills[@]}"; do
  [[ -f ".agents/skills/${skill}/SKILL.md" ]] || {
    echo "[context-surface] Missing retained skill: $skill" >&2
    exit 1
  }
done

for hook_file in "${retained_hook_files[@]}"; do
  [[ -f "$hook_file" ]] || {
    echo "[context-surface] Missing retained hook file: $hook_file" >&2
    exit 1
  }
done

for skill in "${removed_skills[@]}"; do
  if find ".agents/skills/${skill}" -type f ! -name '.DS_Store' 2>/dev/null | grep -q .; then
    echo "[context-surface] Removed skill files still present: $skill" >&2
    exit 1
  fi
done

spec_surface_paths=()
if [[ -d "docs/specs" ]]; then
  while IFS= read -r file; do
    spec_surface_paths+=("$file")
  done < <(find "docs/specs" -maxdepth 1 -type f -name '*.md' | sort)

  if find "docs/specs" -mindepth 1 -type d | grep -q .; then
    echo "[context-surface] docs/specs must stay flat and active-only" >&2
    exit 1
  fi
fi

surface_paths=(
  "README.md"
  "AGENTS.md"
  "PLANS.md"
  "docs/fallbacks.md"
  "docs/api-map.md"
  "package.json"
  "hooks.json"
  "scripts/checks.sh"
  "scripts/check-context-surface.sh"
  "src/AGENTS.md"
  "src/harness/AGENTS.md"
  ".agents/skills"
  ".agents/hooks"
)

legacy_surface_paths=(
  "README.md"
  "AGENTS.md"
  "docs/fallbacks.md"
  "docs/api-map.md"
  "package.json"
  "hooks.json"
  "scripts/checks.sh"
  "src/AGENTS.md"
  "src/harness/AGENTS.md"
  ".agents/skills"
  ".agents/hooks"
)

removed_surface_pattern='WORKFLOW\.md|issue:create|issue:run|test:controlplane|harness:agent|controlplane-operator|scripts/issues|scripts/work|abb:issue-kind|\.agent-work/|repomix:audit|repomix:full|scripts/repomix-handoff\.sh|\.repomix/'

existing_surface_paths=()
for path in "${surface_paths[@]}"; do
  [[ -e "$path" ]] && existing_surface_paths+=("$path")
done
existing_surface_paths+=("${spec_surface_paths[@]}")

text_surface_paths=(
  "README.md"
  "AGENTS.md"
  "PLANS.md"
  "docs/fallbacks.md"
  "docs/api-map.md"
  "src/AGENTS.md"
  "src/harness/AGENTS.md"
)

existing_text_surface_paths=()
for path in "${text_surface_paths[@]}"; do
  [[ -e "$path" ]] && existing_text_surface_paths+=("$path")
done
existing_text_surface_paths+=("${spec_surface_paths[@]}")

existing_legacy_surface_paths=()
for path in "${legacy_surface_paths[@]}"; do
  [[ -e "$path" ]] && existing_legacy_surface_paths+=("$path")
done

! rg -n 'docs/project\.md' "${existing_text_surface_paths[@]}" >/dev/null
! rg -n "$stale_doc_pattern" "${existing_text_surface_paths[@]}" >/dev/null
! rg -n "$stale_check_pattern" "${existing_text_surface_paths[@]}" >/dev/null
! rg -n "$stale_tracking_pattern" "${existing_text_surface_paths[@]}" >/dev/null
! rg -n 'docs/decisions' "${existing_text_surface_paths[@]}" >/dev/null
! rg -n "$removed_surface_pattern" "${existing_legacy_surface_paths[@]}" >/dev/null

echo "[context-surface] OK"
