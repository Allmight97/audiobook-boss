#!/usr/bin/env bash
set -euo pipefail

lockfile="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/bun.lock"
version="$(sed -n 's/^    "bun": \["bun@\([0-9][^"]*\)".*/\1/p' "${lockfile}")"
if [ -z "${version}" ]; then
	printf 'error: no locked bun devDependency in %s\n' "${lockfile}" >&2
	exit 1
fi
printf '%s\n' "${version}"
