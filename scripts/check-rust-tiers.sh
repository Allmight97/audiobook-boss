#!/usr/bin/env bash
# Fails when a Rust tier depends on a crate its tier forbids (crates/AGENTS.md):
# the engine and the pure cores never depend on a UI toolkit, and the pure
# cores never depend on FFmpeg, a credential store, or the engine.
# Usage: bash scripts/check-rust-tiers.sh
set -euo pipefail
cd "$(dirname "$0")/.."

ui_toolkit='^(tauri|tauri-.*|wry|tao)$'
core_forbidden="$ui_toolkit|^(ffmpeg-next|ffmpeg-sys-next|keyring|keyring-core|.*-keyring-store|abb-engine)$"
cores=(
	abb-audible-core abb-media-core abb-metadata-core
	abb-output-artifact-core abb-processing-core abb-remote-source-core
)
failed=0

check() {
	local crate="$1" forbidden="$2" hits
	hits="$(
		cargo tree --locked -p "$crate" -e normal --target all --prefix none --format '{p}' |
			awk '{ print $1 }' | sort -u | grep -E "$forbidden" || true
	)"
	if [[ -n "$hits" ]]; then
		echo "[check-rust-tiers] $crate must not depend on: $(echo "$hits" | tr '\n' ' ')"
		failed=1
	fi
}

check abb-engine "$ui_toolkit"
for core in "${cores[@]}"; do
	check "$core" "$core_forbidden"
done

if [[ "$failed" -eq 0 ]]; then
	echo "[check-rust-tiers] OK"
fi
exit "$failed"
