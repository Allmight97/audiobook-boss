# shellcheck shell=bash
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

abb_dev() {
	local state="${STATE}/$1"
	shift
	bash "${repo_root}/scripts/abb-dev.sh" --state-dir "${state}" "$@"
}

format_tag() {
	ffprobe -hide_banner -loglevel error -show_entries "format_tags=$2" -of default=nw=1:nk=1 "$1"
}

fail() {
	printf 'FAIL: %s\n' "$*" >&2
	exit 1
}
