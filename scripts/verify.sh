#!/usr/bin/env bash
set -euo pipefail

# Same lanes as CI. The lane list lives only in this file.
#   bash scripts/verify.sh            # every lane this OS supports
#   bash scripts/verify.sh frontend   # one or more lanes
#   bash scripts/verify.sh supply-chain  # advisories and licenses (named only)
#
# Lanes run one after another and keep going after a failure. Each lane prints
# PASS or FAIL. The exit code is nonzero if any lane failed.
# Before engine/media/host/apple/decrypt, setup.sh --check rust must pass. Core checks
# cargo, rustfmt, clippy, and uvx only (no apt). Missing tools fail that lane
# at once and print the exact setup command.

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

core_crates=(
	-p abb-audible-core -p abb-media-core -p abb-metadata-core
	-p abb-output-artifact-core -p abb-processing-core -p abb-remote-source-core
)

usage() {
	cat <<'USAGE'
Usage: bash scripts/verify.sh [lane ...] | --changed <base>

Lanes: frontend, core, engine, media, host, apple, decrypt, tooling, supply-chain
No argument runs every lane this OS supports (apple is skipped on Linux).
supply-chain runs when named or selected by changed dependency/policy files.
--changed runs only affected lanes without a current passing input receipt.
USAGE
}

is_darwin() {
	[[ "$(uname -s)" == Darwin ]]
}

default_lanes=(frontend core engine media host apple decrypt tooling)
all_lanes=("${default_lanes[@]}" supply-chain)
if [[ "${1:-}" == --list ]]; then
 printf '%s\n' "${all_lanes[@]}"
 exit 0
fi
if [[ "${1:-}" == --help || "${1:-}" == -h ]]; then
 usage
 exit 0
fi
eval "$(scripts/setup.sh --print-env)"

lane_known() {
	local name="$1" lane
	for lane in "${all_lanes[@]}"; do
		if [[ "${lane}" == "${name}" ]]; then
			return 0
		fi
	done
	return 1
}

setup_bin="${repo_root}/scripts/setup.sh"

require_frontend() {
	if bash "${setup_bin}" --check frontend; then
		return 0
	fi
	printf 'Install with: bash scripts/setup.sh frontend\n' >&2
	return 1
}

require_rust() {
	if bash "${setup_bin}" --check rust; then
		return 0
	fi
	printf 'Install with: bash scripts/setup.sh rust\n' >&2
	return 1
}

# Core crate tests do not need GTK, FFmpeg CLI, or the sidecar. The dedicated
# CI core job has no apt; do not require the full rust --check there.
require_core() {
	local missing=0
	if ! cargo --version >/dev/null 2>&1; then
		printf 'MISSING cargo\n' >&2
		missing=1
	fi
	if ! cargo fmt --version >/dev/null 2>&1; then
		printf 'MISSING rustfmt\n' >&2
		missing=1
	fi
	if ! cargo clippy -V >/dev/null 2>&1; then
		printf 'MISSING clippy\n' >&2
		missing=1
	fi
	if ! command -v uvx >/dev/null 2>&1; then
		printf 'MISSING uvx\n' >&2
		missing=1
	fi
	if [[ "${missing}" -ne 0 ]]; then
		printf 'Install with: bash scripts/setup.sh rust\n' >&2
		return 1
	fi
	return 0
}

lane_frontend() {
	require_frontend
	bun run fmt:check
	bun run lint:check
	bun run bindings:check:runtime-boundary
	bun run typecheck
	bun run knip
	bun run test
}

lane_core() {
	require_core
	cargo fmt --all -- --check
	bash scripts/check-rust-tiers.sh
	bash scripts/check-rust-complexity.sh
	cargo test --locked "${core_crates[@]}"
	cargo clippy --locked "${core_crates[@]}" --all-targets -- -D warnings
}

lane_engine() {
	require_rust
	cargo fmt --all -- --check
	cargo test --locked -p abb-engine --features bundled-ffmpeg --lib -- \
		--skip test_cases::integration_media --skip test_cases::integration_decrypt
	cargo test --locked -p faac-sys
	cargo test --locked -p abb-engine --features bundled-ffmpeg --doc
	cargo test --locked -p abb-engine --features bundled-ffmpeg --test all_tests
	cargo clippy --locked -p abb-engine --features bundled-ffmpeg --all-targets -- -D warnings
}

lane_media() {
	require_rust
	cargo fmt --all -- --check
	cargo test --locked -p abb-engine --features bundled-ffmpeg --lib -- \
		test_cases::integration_media
	bash .agents/skills/verify-abb/run.sh
}

lane_host() {
	require_rust
	cargo fmt --all -- --check
	cargo test --locked -p audiobook-boss --features bundled-ffmpeg
	bash scripts/check-generated-bindings.sh --mode verify
	cargo clippy --locked -p audiobook-boss --features bundled-ffmpeg --all-targets -- -D warnings
}

lane_decrypt() {
	require_rust
	cargo fmt --all -- --check
	dotnet test tools/abb-aaxclean-helper/tests/AbbAaxcleanHelper.Tests/AbbAaxcleanHelper.Tests.csproj \
		--configuration Release --nologo
	cargo test --locked -p abb-engine --features bundled-ffmpeg --lib -- \
		test_cases::integration_decrypt
}

lane_apple() {
	if ! is_darwin; then
		printf 'skipped: macOS only\n'
		return 0
	fi
	require_rust
	cargo fmt --all -- --check
	cargo test --locked -p abb-engine --features bundled-ffmpeg --lib -- apple
}

lane_tooling() {
	local tool
	for tool in actionlint shellcheck uvx bun; do
		if ! command -v "${tool}" >/dev/null 2>&1; then
			printf 'MISSING %s\nInstall with: bash scripts/setup.sh frontend\n' "${tool}" >&2
			return 1
		fi
	done
	bun run test -- --project tooling scripts
	bash scripts/check-workflows.sh
	bun scripts/check-guidance.ts
	bun scripts/check-known-mistakes.ts
	local scripts
	# Globs are expanded against committed files; fail if a directory is empty.
	scripts=(scripts/*.sh .claude/hooks/*.sh .githooks/* .agents/skills/verify-abb/*.sh .agents/skills/verify-abb/features/*.sh)
	shellcheck -S warning "${scripts[@]}"
}

lane_supply_chain() {
	if ! cargo audit --version >/dev/null 2>&1 || ! cargo deny --version >/dev/null 2>&1; then
		printf 'MISSING cargo-audit and/or cargo-deny (CI installs them in the supply-chain job)\n' >&2
		return 1
	fi
 local status=0
 bun audit || status=1
 cargo audit -D warnings || status=1
 cargo deny check licenses sources || status=1
 return "${status}"
}

requested=()
if [[ "${1:-}" == --changed ]]; then
 [[ $# -eq 2 ]] || { usage >&2; exit 2; }
 needed="$(bun scripts/verification.ts needed "$2")"
 while IFS= read -r lane; do
  [[ -z "${lane}" ]] || requested+=("${lane}")
 done <<<"${needed}"
elif [[ $# -eq 0 ]]; then
	requested=("${default_lanes[@]}")
else
	for arg in "$@"; do
		case "${arg}" in
			-h | --help)
				usage
				exit 0
				;;
		esac
		if ! lane_known "${arg}"; then
			printf 'error: unknown lane %s\n' "${arg}" >&2
			usage >&2
			exit 2
		fi
		requested+=("${arg}")
	done
fi

failed=0
results=()

mkdir -p "${repo_root}/.logs/verification"
run_dir="$(mktemp -d "${repo_root}/.logs/verification/run-$(date -u +%Y%m%dT%H%M%SZ)-XXXXXX")"

run_lane() {
 local name="$1" status started elapsed
 local pipeline_status=()
 printf '\n======== %s ========\n' "${name}"
 if ! bun scripts/verification.ts prepare "${run_dir}" "${name}"; then
  results+=("FAIL ${name} (input capture)")
  failed=1
  return
 fi
 started="$(date +%s)"
 if [[ "${name}" == apple ]] && ! is_darwin; then
  printf 'SKIP apple: macOS only\n' | tee "${run_dir}/${name}.log"
  bun scripts/verification.ts record "${run_dir}" "${name}" skip 0
  results+=("SKIP ${name}")
  return
 fi
 set +e
 (
  set -euo pipefail
  "lane_${name//-/_}"
 ) 2>&1 | tee "${run_dir}/${name}.log"
 pipeline_status=("${PIPESTATUS[@]}")
 set -e
 status="${pipeline_status[0]}"
 [[ "${pipeline_status[1]}" -eq 0 ]] || status=1
 elapsed="$(( $(date +%s) - started ))"
 if ! bun scripts/verification.ts record "${run_dir}" "${name}" "${status}" "${elapsed}"; then
  status=1
 fi
 if [[ "${status}" -eq 0 ]]; then
  printf 'PASS %s\n' "${name}"
  results+=("PASS ${name}")
 else
  printf 'FAIL %s\n' "${name}"
  results+=("FAIL ${name}")
  failed=1
 fi
}

for lane in "${requested[@]}"; do
	run_lane "${lane}"
done

printf '\n--------\n'
for line in "${results[@]}"; do
	printf '%s\n' "${line}"
done

printf 'Evidence: %s\n' "${run_dir}"
bun scripts/verification.ts summarize "${run_dir}"
exit "${failed}"
