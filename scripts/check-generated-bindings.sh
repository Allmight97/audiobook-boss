#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"
eval "$(scripts/setup.sh --print-env)"

run_generator() {
	cargo run --locked --manifest-path src-tauri/Cargo.toml --features bundled-ffmpeg,binding-export --bin export_bindings -- "$@"
}

if [[ "${1:-}" == --write ]]; then
	shift
	run_generator "$@"
	exit 0
fi

if [[ $# -ne 0 ]]; then
	printf 'Usage: bash scripts/check-generated-bindings.sh [--write [output-path]]\n' >&2
	exit 2
fi
bindings_file="src/lib/generated/tauri.ts"
temporary="$(mktemp -d)"
trap 'rm -rf "${temporary}"' EXIT
run_generator "${temporary}/tauri.ts"
if [[ ! -f "${bindings_file}" ]]; then
	printf 'Missing bindings: run bun run bindings:generate\n' >&2
	exit 1
fi
if ! cmp -s "${bindings_file}" "${temporary}/tauri.ts"; then
	printf 'Generated bindings are stale: run bun run bindings:generate\n' >&2
	diff -u "${bindings_file}" "${temporary}/tauri.ts" || [[ $? -eq 1 ]]
	exit 1
fi
fixtures_dir="src/lib/generated/fixtures"
if ! diff -ru -x '.*' "${fixtures_dir}" "${temporary}/fixtures"; then
	printf 'Contract fixtures are stale or differ from the recorder: run bun run bindings:generate\n' >&2
	exit 1
fi
printf 'Bindings are up to date.\n'
