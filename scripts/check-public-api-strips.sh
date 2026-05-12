#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

failures=0

compare_block() {
  local name="$1"
  local actual="$2"
  local expected="$3"

  if ! diff -u \
    <(printf '%s\n' "$expected" | sed '/^[[:space:]]*$/d') \
    <(printf '%s\n' "$actual" | sed '/^[[:space:]]*$/d') \
    >/tmp/public-api-strip.diff; then
    echo "[public-api-strips] $name changed; update the matching contract test and AGENTS.md intentionally." >&2
    cat /tmp/public-api-strip.diff >&2
    failures=1
  fi
  rm -f /tmp/public-api-strip.diff
}

extract_export_blocks() {
  awk '
    /^(pub(\([^)]*\))? use|pub use|export \{)/ { capture=1 }
    capture {
      line=$0
      sub(/^[[:space:]]+/, "", line)
      sub(/[[:space:]]+$/, "", line)
      print line
      if ($0 ~ /;$/) {
        capture=0
      }
    }
  ' "$1"
}

output_artifact_exports="$(extract_export_blocks src-tauri/src/output_artifact/mod.rs)"
compare_block "Output Artifact Plan / Commit Public API Strip" "$output_artifact_exports" 'pub(crate) use artifact::derive_output_artifact_path;
pub(crate) use commit::{commit_output_artifact, finalized_output_success, OutputCommitRequest};
pub(crate) use naming::build_output_path;
pub use naming::build_output_path_preview;
pub(crate) use plan::{action_requires_output_write, plan_is_hard_block, OutputPlanLedger};
pub(crate) use review::{enforce_output_plan_review, ensure_output_parent_dirs, OutputPlanReview};
pub use types::{
CollisionPolicy, NamingPreset, OutputCollisionInfo, OutputCollisionKind, OutputKind,
OutputNamingConfig, OutputReviewRequirement, PlannedOutput, PlannedOutputAction,
};
pub(crate) use types::{OutputCollision, ResolvedOutputPlan};'

processing_plan_exports="$(rg "^(pub\\(crate\\) (struct|fn)|pub (struct|fn)) " src-tauri/src/processing/plan.rs || true)"
compare_block "Processing Plan Public API Strip" "$processing_plan_exports" 'pub(crate) struct PlannedProcessingJob {
pub(crate) struct ResolvedProcessingPlan {
pub(crate) fn resolve_preflight_plan(
pub(crate) fn prepare_execution_plan('

metadata_intent_exports="$(extract_export_blocks src-tauri/src/metadata/mod.rs | rg "intent|intent_plan" || true)"
compare_block "Metadata Intent Plan Public API Strip" "$metadata_intent_exports" 'pub use intent::{AlbumSortPatchOp, MetadataIntentPatch, PatchOp};
pub(crate) use intent::{AlbumSortWriteAction, MetadataWritePlan};
pub(crate) use intent_plan::{resolve_effective_processing_metadata, resolve_naming_metadata};'

tauri_client_exports="$(rg "^export (const|type)" src/lib/tauri/client.ts || true)"
compare_block "Tauri Runtime Boundary Public API Strip" "$tauri_client_exports" 'export const tauriClient = {
export const TAURI_COMMAND_NAMES = Object.freeze(
export const TAURI_APP_EVENT_NAMES = Object.freeze([
export type { TauriCommand };'

status_panel_exports="$(extract_export_blocks src/ui/statusPanel/index.ts)"
compare_block "Status Panel Runtime Public API Strip" "$status_panel_exports" 'export {
initStatusPanel,
isStatusPanelProcessing,
pushStatusPanelTransientStatus,
triggerCancelAllFromStatusPanel,
triggerProcessFromStatusPanel,
} from '\''./controller'\'';
export { updateStatusPanelConcurrencyStatus } from '\''./runtimeApi'\'';
export { default as StatusPanelIsland } from '\''./StatusPanelIsland.svelte'\'';'

if [[ "$failures" -ne 0 ]]; then
  exit 1
fi

echo "[public-api-strips] OK"
