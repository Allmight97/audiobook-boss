#!/usr/bin/env bash
# Temporary. Samples WebKitWebProcess CPU while the built app sits idle.
# Usage: bash scripts/measure-idle-webkit.sh [path-to-audiobook-boss]
set -euo pipefail

bin="${1:-}"
if [[ -z "${bin}" ]]; then
	for candidate in target/debug/audiobook-boss src-tauri/target/debug/audiobook-boss; do
		if [[ -x "${candidate}" ]]; then
			bin="${candidate}"
			break
		fi
	done
fi
if [[ -z "${bin}" || ! -x "${bin}" ]]; then
	printf 'No audiobook-boss binary. Pass its path.\n' >&2
	exit 1
fi

duration="${DURATION:-60}"
label="${MODE_LABEL:-default}"
clk="$(getconf CLK_TCK)"

ticks() {
	awk '{ print $14 + $15 }' "/proc/${1}/stat"
}

# WebKit's helper is not always a direct child of the app.
webkit_pids() {
	local app_pid="$1"
	local pid comm ppid stat
	for pid in /proc/[0-9]*; do
		pid="${pid#/proc/}"
		# A process can exit between the listing and the read.
		[[ -r "/proc/${pid}/comm" ]] || continue
		comm="$(cat "/proc/${pid}/comm")"
		case "${comm}" in
			WebKitWebProces*) ;;
			*) continue ;;
		esac
		ppid="${pid}"
		while [[ "${ppid}" != "0" && "${ppid}" != "1" ]]; do
			if [[ "${ppid}" == "${app_pid}" ]]; then
				printf '%s\n' "${pid}"
				break
			fi
			[[ -r "/proc/${ppid}/stat" ]] || break
			stat="$(cat "/proc/${ppid}/stat")"
			ppid="$(printf '%s\n' "${stat}" | awk '{ print $4 }')"
		done
	done
}

setsid xvfb-run -a -s '-screen 0 1600x1000x24' "${bin}" >/tmp/abb-idle-webkit.log 2>&1 &
app_pid=$!
trap 'kill -- -"${app_pid}" >/dev/null 2>&1 || true; wait "${app_pid}" >/dev/null 2>&1 || true' EXIT

ready=""
for _ in $(seq 1 30); do
	if webkit_pids "${app_pid}" | grep -q .; then
		ready=1
		break
	fi
	if ! kill -0 "${app_pid}" 2>/dev/null; then
		printf 'App exited before WebKit started. Log:\n' >&2
		cat /tmp/abb-idle-webkit.log >&2 || true
		exit 1
	fi
	sleep 1
done
if [[ -z "${ready}" ]]; then
	printf 'WebKitWebProcess did not appear. Log:\n' >&2
	cat /tmp/abb-idle-webkit.log >&2 || true
	exit 1
fi

sleep 2
start_ticks=0
pids="$(webkit_pids "${app_pid}")"
for pid in ${pids}; do
	start_ticks=$((start_ticks + $(ticks "${pid}")))
done
sleep "${duration}"
end_ticks=0
end_pids="$(webkit_pids "${app_pid}")"
for pid in ${end_pids}; do
	end_ticks=$((end_ticks + $(ticks "${pid}")))
done
delta=$((end_ticks - start_ticks))
# Percent of one core, summed across WebKit processes.
percent="$(awk -v delta="${delta}" -v clk="${clk}" -v seconds="${duration}" 'BEGIN { printf "%.2f", (delta / clk) / seconds * 100 }')"
printf 'mode=%s seconds=%s webkit_cpu_percent=%s pids=%s\n' "${label}" "${duration}" "${percent}" "$(echo ${end_pids} | tr '\n' ',')"
