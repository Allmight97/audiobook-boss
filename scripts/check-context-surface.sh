#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

active_files=(
  "README.md"
  "AGENTS.md"
  "WORKFLOW.md"
  "docs/fallbacks.md"
)

retained_skills=(
  "audiobook-metadata"
  "contract-guardrails"
  "controlplane-operator"
  "job-registry-and-progress"
  "lib-research"
  "path-security-validation"
  "tauri-command-conventions"
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
)

stale_doc_pattern='docs/README\.md|docs/AGENTS\.md|docs/agent-execution\.md|docs/browser-harness\.md|docs/skills-audit\.md|docs/specs/technical-reference\.md|docs/verification\.md|docs/workloop\.md'
stale_check_pattern='check-docs-routing|check-skills-routing'

for file in "${active_files[@]}"; do
  [[ -f "$file" ]] || {
    echo "[context-surface] Missing active file: $file" >&2
    exit 1
  }
done

for skill in "${retained_skills[@]}"; do
  [[ -f ".agents/skills/${skill}/SKILL.md" ]] || {
    echo "[context-surface] Missing retained skill: $skill" >&2
    exit 1
  }
done

for skill in "${removed_skills[@]}"; do
  if find ".agents/skills/${skill}" -type f ! -name '.DS_Store' 2>/dev/null | grep -q .; then
    echo "[context-surface] Removed skill files still present: $skill" >&2
    exit 1
  fi
done

! rg -n 'docs/project\.md' README.md AGENTS.md WORKFLOW.md .agents/skills scripts/issues package.json scripts/checks.sh scripts/work/cli.ts >/dev/null
! rg -n "$stale_doc_pattern" README.md AGENTS.md WORKFLOW.md .agents/skills scripts/issues package.json scripts/checks.sh scripts/work/cli.ts >/dev/null
! rg -n "$stale_check_pattern" README.md AGENTS.md WORKFLOW.md .agents/skills scripts/issues package.json scripts/checks.sh >/dev/null
! rg -n 'docs/decisions' README.md AGENTS.md WORKFLOW.md .agents/skills scripts/issues >/dev/null

echo "[context-surface] OK"
