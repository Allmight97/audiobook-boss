# shellcheck shell=bash
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

tone() {
	local out="$1"
	shift
	ffmpeg -hide_banner -loglevel error -f lavfi -i sine=frequency=440:sample_rate=44100:duration=1 \
		-c:a aac -b:a 64k "$@" -y "${out}"
}

abb_dev() {
	local state="${STATE}/$1"
	shift
	bash "${repo_root}/scripts/abb-dev.sh" --state-dir "${state}" "$@"
}

refused() {
	local log="$1" message="$2" status=0
	shift 2
	abb_dev "$@" >"${log}" 2>&1 || status=$?
	[[ "${status}" -eq 1 ]] || fail "expected exit 1, got ${status} (log: ${log})"
	grep -qF -- "${message}" "${log}" || fail "expected '${message}' (log: ${log})"
}

format_tag() {
	ffprobe -hide_banner -loglevel error -show_entries "format_tags=$2" -of default=nw=1:nk=1 "$1"
}

fail() {
	printf 'FAIL: %s\n' "$*" >&2
	exit 1
}
