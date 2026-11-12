#!/usr/bin/env bash
# Fails when a Rust tier depends on a crate its tier forbids:
# the engine and the pure cores never depend on a UI toolkit, and the pure
# cores never depend on FFmpeg, a credential store, or the engine. Only
# abb-ffmpeg depends on ffmpeg-sys-next directly or names it in source, and
# no crate uses the ffmpeg-next wrapper.
# Usage: bash scripts/check-rust-tiers.sh
set -euo pipefail
cd "$(dirname "$0")/.."

ui_toolkit='^(tauri|tauri-.*|wry|tao)$'
core_forbidden="$ui_toolkit|^(abb-ffmpeg|ffmpeg-next|ffmpeg-sys-next|keyring|keyring-core|.*-keyring-store|abb-engine)$"
cores=()
for manifest in crates/abb-*-core/Cargo.toml; do
	cores+=("$(basename "$(dirname "$manifest")")")
done
failed=0

check() {
	local crate="$1" forbidden="$2" tree hits
	tree="$(cargo tree --locked -p "$crate" -e normal --target all --prefix none --format '{p}')"
	# grep exits 1 when no forbidden crate matches; any other failure fails.
	hits="$(awk '{ print $1 }' <<<"$tree" | sort -u | grep -E "$forbidden" || [[ $? -eq 1 ]])"
	if [[ -n "$hits" ]]; then
		echo "[check-rust-tiers] $crate must not depend on: $(echo "$hits" | tr '\n' ' ')"
		failed=1
	fi
}

# Fails when a crate lists a forbidden crate among its own dependencies.
check_direct() {
	local crate="$1" forbidden="$2" tree hits
	tree="$(cargo tree --locked -p "$crate" -e normal --target all --depth 1 --prefix none --format '{p}')"
	# grep exits 1 when no forbidden crate matches; any other failure fails.
	hits="$(awk 'NR > 1 { print $1 }' <<<"$tree" | sort -u | grep -E "$forbidden" || [[ $? -eq 1 ]])"
	if [[ -n "$hits" ]]; then
		echo "[check-rust-tiers] $crate must not depend directly on: $(echo "$hits" | tr '\n' ' '). Fix: call abb_ffmpeg."
		failed=1
	fi
}

check abb-engine "$ui_toolkit"
check_direct abb-engine '^ffmpeg-(sys-)?next$'
check_direct audiobook-boss '^ffmpeg-(sys-)?next$'
check abb-ffmpeg '^(abb-engine|abb-.*-core|audiobook-boss|faac-sys)$'
for core in "${cores[@]}"; do
	check "$core" "$core_forbidden"
done

raw_ffmpeg_users="$(git grep -lE 'ffmpeg_(sys_)?next' -- 'crates/*.rs' 'src-tauri/*.rs' ':!crates/abb-ffmpeg' || [[ $? -eq 1 ]])"
if [[ -n "$raw_ffmpeg_users" ]]; then
	echo "[check-rust-tiers] raw FFmpeg outside abb-ffmpeg. Fix: call abb_ffmpeg, and add the operation there if it is missing:"
	echo "$raw_ffmpeg_users"
	failed=1
fi

if [[ "$failed" -eq 0 ]]; then
	echo "[check-rust-tiers] OK"
fi
exit "$failed"
