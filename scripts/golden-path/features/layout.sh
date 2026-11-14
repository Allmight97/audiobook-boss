#!/usr/bin/env bash
# Layout: the default export path follows Author/Series/Book N - Title, and a
# hostile tag or template never writes outside the output folder.
set -euo pipefail
source "$(dirname "$0")/../lib.sh"

root="${INPUTS}/layout"
mkdir -p "${root}"
tone "${INPUTS}/layout-dune.m4b" -metadata title=Dune -metadata artist='Frank Herbert'

abb_dev layout-series --set series=Dune --set series-part=1 --out "${root}/series" --export \
	"${INPUTS}/layout-dune.m4b" >"${EVIDENCE}/layout-series.txt"
[[ -f "${root}/series/Frank Herbert/Dune/Book 1 - Dune/Book 1 - Dune.m4b" ]] ||
	fail "series export is not at Author/Series/Book N - Title"

abb_dev layout-plain --out "${root}/plain" --export "${INPUTS}/layout-dune.m4b" \
	>"${EVIDENCE}/layout-plain.txt"
[[ -f "${root}/plain/Frank Herbert/Dune/Dune.m4b" ]] || fail "export is not at Author/Title"

tone "${INPUTS}/layout-escape.m4b" -metadata title='../../Escape: Part/One' -metadata artist='..'
abb_dev layout-hostile --set series=.. --template '{author}/{series}/{title}' \
	--out "${root}/hostile" --export "${INPUTS}/layout-escape.m4b" >"${EVIDENCE}/layout-hostile.txt"
hostile_exports="$(find "${root}/hostile" -type f)"
hostile_export="${root}/hostile/_/_/.. .. Escape - Part One.m4b"
[[ "${hostile_exports}" == "${hostile_export}" ]] ||
	fail "hostile tags did not sanitize to exactly one export: ${hostile_exports}"
[[ "$(format_tag "${hostile_export}" title)" == '../../Escape: Part/One' ]] ||
	fail "sanitizing the path changed the title tag"

refused "${EVIDENCE}/layout-template.txt" 'Custom template path traversal is not allowed' \
	layout-template --template '../{title}' --out "${root}/template" --export "${INPUTS}/layout-dune.m4b"
leaked="$(find "${root}" -mindepth 1 -maxdepth 1 -not -name series -not -name plain \
	-not -name hostile -not -name template)"
[[ -z "${leaked}" ]] || fail "an export escaped the output folder: ${leaked}"
[[ -d "${root}/template" ]] || fail "the export folder abb-dev made is missing"
[[ -z "$(find "${root}/template" -mindepth 1)" ]] || fail "a refused template left files or folders"

ln -s "${INPUTS}/layout-dune.m4b" "${INPUTS}/layout-link.m4b"
refused "${EVIDENCE}/layout-link.txt" 'Symlinks are not supported' layout-link "${INPUTS}/layout-link.m4b"
