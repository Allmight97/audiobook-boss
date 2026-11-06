#!/usr/bin/env bash
# Corpus: the owner's real audiobooks, copied, go through import, encode,
# Preserve, and re-import, and keep their title, chapters, and length. Opt in
# with ABB_CORPUS_DIR=<folder>; run it by name (run.sh corpus). Never part of
# the default run, and never writes into the corpus folder.
set -euo pipefail
source "$(dirname "$0")/../lib.sh"

if [[ -z "${ABB_CORPUS_DIR:-}" ]]; then
	printf 'SKIP: set ABB_CORPUS_DIR to a folder of audiobooks to run this feature\n'
	exit 77
fi
[[ -d "${ABB_CORPUS_DIR}" ]] || fail "ABB_CORPUS_DIR is not a folder: ${ABB_CORPUS_DIR}"

corpus_listing() {
	find "${ABB_CORPUS_DIR}" -type f -exec ls -l {} + | LC_ALL=C sort
}

# A raw ADTS stream has no length header; ffprobe estimates it from the bit
# rate and was 1.1% short on a 5-minute probe, so that one is measured by decoding.
duration_of() {
	if [[ "$1" == *.aac ]]; then
		ffmpeg -hide_banner -nostats -i "$1" -f null - 2>&1 |
			awk -F'time=' '/time=/ { t = $2 } END { split(t, p, " "); split(p[1], h, ":"); print h[1] * 3600 + h[2] * 60 + h[3] }'
	else
		ffprobe -hide_banner -loglevel error -show_entries format=duration -of default=nw=1:nk=1 "$1"
	fi
}

chapter_lines() {
	ffprobe -hide_banner -loglevel error -show_chapters \
		-show_entries chapter=start_time:chapter_tags=title -of csv=p=0 "$1"
}

# Codec priming and padding move a length by a few frames, not by seconds:
# synthetic books of 1 minute to 1 hour (MP3, AAC, FLAC, WAV; encode and
# Preserve) came out within 0.01 s of the source. 0.25 s allows 2112 AAC priming
# samples at 22.05 kHz (0.1 s) with margin, and does not grow with the book.
LENGTH_TOLERANCE_SECONDS=0.25

same_length() {
	awk -v a="$1" -v b="$2" -v t="${LENGTH_TOLERANCE_SECONDS}" 'BEGIN { d = a - b; if (d < 0) d = -d; exit d > t }'
}

same_length 36000 35999.9 || fail "same_length rejects a 0.1 s drift"
same_length 36000 35821 && fail "same_length accepts a 179 s truncation of a 10-hour book"

same_chapters() {
	[[ "$(wc -l <"$1")" -eq "$(wc -l <"$2")" ]] || return 1
	paste -d'|' "$1" "$2" | awk -F'|' '
		{
			split($1, x, ","); split($2, y, ",")
			titlex = substr($1, length(x[1]) + 2); titley = substr($2, length(y[1]) + 2)
			d = x[1] - y[1]; if (d < 0) d = -d
			if (d > 0.1 || titlex != titley) bad = 1
		}
		END { exit bad }'
}

# compare_output <label> <source-copy> <output> <expected-title>
compare_output() {
	local label="$1" source="$2" output="$3" title="$4" ok=0
	[[ "$(format_tag "${output}" title)" == "${title}" ]] || {
		printf 'FAIL %s: title is not %s\n' "${label}" "${title}"
		ok=1
	}
	same_length "$(duration_of "${source}")" "$(duration_of "${output}")" || {
		printf 'FAIL %s: length %s -> %s\n' "${label}" "$(duration_of "${source}")" "$(duration_of "${output}")"
		ok=1
	}
	chapter_lines "${source}" >"${work}/source.chapters"
	chapter_lines "${output}" >"${work}/output.chapters"
	same_chapters "${work}/source.chapters" "${work}/output.chapters" || {
		printf 'FAIL %s: chapters differ\n' "${label}"
		ok=1
	}
	return "${ok}"
}

# run_mode <n> <mode> <copy> <title> <abb-dev args...>: export, compare, re-import.
run_mode() {
	local n="$1" mode="$2" copy="$3" title="$4"
	shift 4
	local out="${INPUTS}/corpus-out/${n}-${mode}"
	abb_dev "corpus-${n}-${mode}" "$@" --out "${out}" --export "${copy}" \
		>"${EVIDENCE}/corpus-${n}-${mode}.txt" 2>&1 || {
		printf 'FAIL %s %s: export failed (see corpus-%s-%s.txt)\n' "${n}" "${mode}" "${n}" "${mode}"
		return 1
	}
	local produced
	produced="$(find "${out}" -type f \( -name '*.m4b' -o -name '*.mp3' \))"
	[[ -n "${produced}" && "$(printf '%s\n' "${produced}" | wc -l)" -eq 1 ]] || {
		printf 'FAIL %s %s: expected one output, found: %s\n' "${n}" "${mode}" "${produced}"
		return 1
	}
	compare_output "${n} ${mode}" "${copy}" "${produced}" "${title}" || return 1
	abb_dev "corpus-${n}-${mode}-again" --json "${produced}" >"${EVIDENCE}/corpus-${n}-${mode}-again.json" ||
		{
			printf 'FAIL %s %s: the output does not import again\n' "${n}" "${mode}"
			return 1
		}
	grep -q '"isValid": true' "${EVIDENCE}/corpus-${n}-${mode}-again.json" || {
		printf 'FAIL %s %s: the output re-imports as invalid\n' "${n}" "${mode}"
		return 1
	}
}

work="${INPUTS}/corpus-work"
mkdir -p "${work}" "${INPUTS}/corpus-out"
before="$(corpus_listing)"

total=0
failed=0
while IFS= read -r -d '' original; do
	total=$((total + 1))
	n="${total}"
	name="$(basename "${original}")"
	extension="$(printf '%s' "${name##*.}" | tr '[:upper:]' '[:lower:]')"
	mkdir -p "${work}/${n}"
	copy="${work}/${n}/${name}"
	cp -p "${original}" "${copy}"
	title="$(format_tag "${copy}" title)"
	title_args=()
	if [[ -z "${title}" ]]; then
		title="Corpus ${n}"
		title_args=(--set "title=${title}")
	fi
	printf '== %s: %s\n' "${n}" "${name}"
	file_failed=0
	run_mode "${n}" encode "${copy}" "${title}" ${title_args[@]+"${title_args[@]}"} --intent encode --format m4b ||
		file_failed=1
	case "${extension}" in
	mp3) preserve_format=mp3 ;;
	m4b | m4a) preserve_format=m4b ;;
	*) preserve_format="" ;;
	esac
	if [[ -n "${preserve_format}" ]]; then
		run_mode "${n}" preserve "${copy}" "${title}" ${title_args[@]+"${title_args[@]}"} --intent preserve \
			--format "${preserve_format}" || file_failed=1
	fi
	failed=$((failed + file_failed))
done < <(find "${ABB_CORPUS_DIR}" -type f \( -iname '*.m4b' -o -iname '*.m4a' -o -iname '*.mp3' \
	-o -iname '*.flac' -o -iname '*.mka' -o -iname '*.aac' -o -iname '*.wav' \) -print0 | LC_ALL=C sort -z)

[[ "${total}" -gt 0 ]] || fail "no audio files under ${ABB_CORPUS_DIR}"
[[ "$(corpus_listing)" == "${before}" ]] || fail "the corpus folder changed during the run"
printf '%s files, %s failed\n' "${total}" "${failed}"
[[ "${failed}" -eq 0 ]]
