#!/usr/bin/env bash
set -euo pipefail

section() {
  printf '\n== %s ==\n' "$1"
}

section "Repo"
pwd
git status --short
git rev-parse --abbrev-ref HEAD

section "Tool Versions"
if command -v bun >/dev/null 2>&1; then
  bun --revision
else
  echo "bun: missing"
fi
if command -v rustc >/dev/null 2>&1; then
  rustc --version
else
  echo "rustc: missing"
fi
if command -v cargo >/dev/null 2>&1; then
  cargo --version
else
  echo "cargo: missing"
fi
if command -v rustup >/dev/null 2>&1; then
  rustup --version | head -1
fi

section "Bun Drift"
if command -v bun >/dev/null 2>&1; then
  bun outdated || true
fi

section "Bun Supply Chain"
if command -v bun >/dev/null 2>&1; then
  bun audit || true
  bun pm untrusted || true
fi

section "Rust Toolchain"
if command -v rustup >/dev/null 2>&1; then
  rustup show active-toolchain || true
  rustup check || true
fi

section "Rust Security"
if command -v cargo >/dev/null 2>&1; then
  cargo audit -D warnings || true
fi

section "Cargo Compatible Update Preview"
if command -v cargo >/dev/null 2>&1; then
  cargo update --dry-run 2>&1 | sed -n '1,220p' || true
fi

section "Workflow Action Pins"
if command -v rg >/dev/null 2>&1; then
  rg -n "uses:|setup-bun|dtolnay/rust-toolchain" .github/workflows || true
else
  grep -R -n -E "uses:|setup-bun|dtolnay/rust-toolchain" .github/workflows || true
fi

