#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

ui_toolkit='^(tauri|tauri-.*|wry|tao)$'
test_fixtures='^abb-test-fixtures$'
core_forbidden="$ui_toolkit|$test_fixtures|^(abb-ffmpeg|ffmpeg-next|ffmpeg-sys-next|keyring|keyring-core|.*-keyring-store|abb-engine)$"
cores=()
for manifest in crates/abb-*-core/Cargo.toml; do
	cores+=("$(basename "$(dirname "$manifest")")")
done
failed=0

check() {
	local crate="$1" forbidden="$2" tree hits
	tree="$(cargo tree --locked -p "$crate" -e normal --target all --prefix none --format '{p}')"
	hits="$(awk '{ print $1 }' <<<"$tree" | sort -u | grep -E "$forbidden" || [[ $? -eq 1 ]])"
	if [[ -n "$hits" ]]; then
		echo "[check-rust-tiers] $crate must not depend on: $(echo "$hits" | tr '\n' ' ')"
		failed=1
	fi
}

check_direct() {
	local crate="$1" forbidden="$2" tree hits
	tree="$(cargo tree --locked -p "$crate" -e normal --target all --depth 1 --prefix none --format '{p}')"
	hits="$(awk 'NR > 1 { print $1 }' <<<"$tree" | sort -u | grep -E "$forbidden" || [[ $? -eq 1 ]])"
	if [[ -n "$hits" ]]; then
		echo "[check-rust-tiers] $crate must not depend directly on: $(echo "$hits" | tr '\n' ' '). Fix: call abb_ffmpeg."
		failed=1
	fi
}

check abb-engine "$ui_toolkit|$test_fixtures"
check audiobook-boss "$test_fixtures"
check abb-test-fixtures '^(abb-engine|abb-ffmpeg|abb-.*-core|audiobook-boss)$'
check_direct abb-engine '^ffmpeg-(sys-)?next$'
check_direct audiobook-boss '^ffmpeg-(sys-)?next$'
check abb-ffmpeg '^(abb-engine|abb-.*-core|abb-test-fixtures|audiobook-boss|faac-sys)$'
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
