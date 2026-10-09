# shellcheck shell=bash
# Shared by run.sh and features/*.sh; sourced, not run.
# run.sh exports EVIDENCE, STATE, and INPUTS. A feature passes when its
# script exits 0; any failing command fails it (set -euo pipefail).

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"

# tone <output> [ffmpeg output options...]: one second of sine, AAC unless overridden.
tone() {
	local out="$1"
	shift
	ffmpeg -hide_banner -loglevel error -f lavfi -i sine=frequency=440:sample_rate=44100:duration=1 \
		-c:a aac -b:a 64k "$@" -y "${out}"
}

# abb_dev <state-name> [abb-dev args...]: each call gets its own engine state.
abb_dev() {
	local state="${STATE}/$1"
	shift
	bash "${repo_root}/scripts/abb-dev.sh" --state-dir "${state}" "$@"
}

# format_tag <file> <tag>: one format tag, read back with ffprobe.
format_tag() {
	ffprobe -hide_banner -loglevel error -show_entries "format_tags=$2" -of default=nw=1:nk=1 "$1"
}

fail() {
	printf 'FAIL: %s\n' "$*" >&2
	exit 1
}
