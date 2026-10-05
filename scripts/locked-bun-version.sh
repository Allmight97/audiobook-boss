#!/usr/bin/env bash
set -euo pipefail

# Prints the Bun version bun.lock resolves for the `bun` devDependency.
# Bun updates arrive through Dependabot's 10-day cooldown like any other
# dependency; CI and agent setup install the version this prints.
# Usage: bash scripts/locked-bun-version.sh

lockfile="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/bun.lock"
version="$(sed -n 's/^    "bun": \["bun@\([0-9][^"]*\)".*/\1/p' "${lockfile}")"
if [ -z "${version}" ]; then
	printf 'error: no locked bun devDependency in %s\n' "${lockfile}" >&2
	exit 1
fi
printf '%s\n' "${version}"
