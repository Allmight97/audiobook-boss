#!/usr/bin/env bash
# Export: an encoded export in each output format has the right extension, container, and codec.
set -euo pipefail
source "$(dirname "$0")/../lib.sh"

tone "${INPUTS}/export.m4b" -metadata title=VerifyExport
for run in "m4b m4b aac mov,mp4,m4a,3gp,3g2,mj2" "m4aOpus m4a opus mov,mp4,m4a,3gp,3g2,mj2" "mkaOpus mka opus matroska,webm"; do
	read -r format extension want_codec want_container <<<"${run}"
	out="${INPUTS}/export-out-${format}"
	abb_dev "export-${format}" --json --format "${format}" --intent encode --bitrate 64 \
		--template '{title}' --out "${out}" --export "${INPUTS}/export.m4b" >"${EVIDENCE}/export-${format}.json"
	output="${out}/VerifyExport.${extension}"
	[[ -f "${output}" ]] || fail "${format} export did not write ${output}"
	codec="$(ffprobe -hide_banner -loglevel error -select_streams a:0 -show_entries stream=codec_name \
		-of default=nw=1:nk=1 "${output}")"
	[[ "${codec}" == "${want_codec}" ]] || fail "${format} audio codec is '${codec}', expected ${want_codec}"
	container="$(ffprobe -hide_banner -loglevel error -show_entries format=format_name \
		-of default=nw=1:nk=1 "${output}")"
	[[ "${container}" == "${want_container}" ]] || fail "${format} container is '${container}', expected ${want_container}"
done
