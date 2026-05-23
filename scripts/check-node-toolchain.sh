#!/usr/bin/env bash
# Verifies ABB's canonical Node-backed tooling runtime.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
expected_major="$(tr -d '[:space:]' < "$repo_root/.node-version")"

if [[ -z "$expected_major" || ! "$expected_major" =~ ^[0-9]+$ ]]; then
  echo "[node-toolchain] .node-version must contain a Node major version, found '$expected_major'." >&2
  exit 1
fi

if ! command -v node >/dev/null 2>&1; then
  echo "[node-toolchain] Node ${expected_major}.x LTS is required for Node-backed tooling, but 'node' was not found." >&2
  echo "[node-toolchain] Install/use Node ${expected_major}.x with a manager that honors .node-version or .nvmrc." >&2
  exit 1
fi

node_version="$(node --version)"
node_major="${node_version#v}"
node_major="${node_major%%.*}"

if [[ "$node_major" != "$expected_major" ]]; then
  echo "[node-toolchain] Node ${expected_major}.x LTS is required for ABB's Node-backed tooling." >&2
  echo "[node-toolchain] Active node is ${node_version}." >&2
  echo "[node-toolchain] Use a node manager that honors .node-version/.nvmrc, then rerun the command." >&2
  exit 1
fi
