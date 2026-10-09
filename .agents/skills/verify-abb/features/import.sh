#!/usr/bin/env bash
# Import: one synthesized M4B becomes one valid title with its artist tag.
set -euo pipefail
source "$(dirname "$0")/../lib.sh"

tone "${INPUTS}/import.m4b" -metadata title=VerifyImport -metadata artist="Verify Author"
abb_dev import --json "${INPUTS}/import.m4b" >"${EVIDENCE}/import.json"
grep -q '"tagArtist": "Verify Author"' "${EVIDENCE}/import.json" ||
	fail "import.json does not show the artist tag"
