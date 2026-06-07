#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
log_dir="$repo_root/.logs"
log_file="$log_dir/tauri-dev.log"
dev_port="1420"

port_listener_pids() {
	lsof -tiTCP:"$dev_port" -sTCP:LISTEN 2>/dev/null | sort -u
}

process_command() {
	ps -p "$1" -o command= 2>/dev/null || true
}

parent_pid() {
	ps -p "$1" -o ppid= 2>/dev/null | tr -d ' ' || true
}

process_cwd() {
	lsof -a -p "$1" -d cwd -Fn 2>/dev/null | awk 'substr($0, 1, 1) == "n" { print substr($0, 2); exit }'
}

read_port_listener_pids() {
	local pid
	pids=()
	while IFS= read -r pid; do
		[[ -n "$pid" ]] && pids+=("$pid")
	done < <(port_listener_pids)
}

is_repo_dev_listener() {
	local pid="$1"
	local command cwd

	command="$(process_command "$pid")"
	cwd="$(process_cwd "$pid")"

	[[ "$cwd" == "$repo_root" ]] || return 1
	[[ "$command" == *"node_modules/.bin/vite"* || "$command" == *"bun run dev"* || "$command" == *" vite"* ]]
}

is_repo_dev_process() {
	local pid="$1"
	local command cwd

	command="$(process_command "$pid")"
	cwd="$(process_cwd "$pid")"

	[[ "$cwd" == "$repo_root" ]] || return 1
	[[ "$command" == *"bun run app:dev:log"* ||
		"$command" == *"bun run tauri dev"* ||
		"$command" == *"node_modules/.bin/tauri dev"* ||
		"$command" == *"aaxclean-helper:publish && bun run dev"* ||
		"$command" == *"bun run dev"* ||
		"$command" == *"node_modules/.bin/vite"* ||
		"$command" == *" vite"* ]]
}

append_unique_pid() {
	local candidate="$1"
	local existing

	for existing in "${dev_process_pids[@]}"; do
		[[ "$existing" == "$candidate" ]] && return 0
	done

	dev_process_pids+=("$candidate")
}

collect_repo_dev_processes_from_pid() {
	local pid="$1"

	while [[ -n "$pid" && "$pid" != "0" && "$pid" != "1" ]]; do
		is_repo_dev_process "$pid" || break
		append_unique_pid "$pid"
		pid="$(parent_pid "$pid")"
	done
}

reclaim_dev_port() {
	local pid command cwd
	local -a pids dev_process_pids
	read_port_listener_pids

	((${#pids[@]} > 0)) || return 0

	for pid in "${pids[@]}"; do
		if ! is_repo_dev_listener "$pid"; then
			command="$(process_command "$pid")"
			cwd="$(process_cwd "$pid")"
			printf 'Port %s is already in use by a non-ABB process; not killing it.\n' "$dev_port" >&2
			printf 'PID: %s\nCWD: %s\nCommand: %s\n' "$pid" "${cwd:-unknown}" "${command:-unknown}" >&2
			printf 'Stop that process or free port %s, then rerun bun run app:dev:log.\n' "$dev_port" >&2
			exit 1
		fi
		collect_repo_dev_processes_from_pid "$pid"
	done

	printf 'Port %s is already held by ABB dev process(es): %s. Replacing them.\n' "$dev_port" "${dev_process_pids[*]}"
	for pid in "${dev_process_pids[@]}"; do
		kill "$pid" 2>/dev/null || true
	done

	for _ in {1..20}; do
		read_port_listener_pids
		((${#pids[@]} == 0)) && return 0
		sleep 0.25
	done

	for pid in "${dev_process_pids[@]}"; do
		if is_repo_dev_process "$pid"; then
			kill -KILL "$pid" 2>/dev/null || true
		fi
	done

	for _ in {1..20}; do
		read_port_listener_pids
		((${#pids[@]} == 0)) && return 0
		sleep 0.25
	done

	printf 'Port %s is still in use after replacing ABB dev server process(es). Rerun lsof -nP -iTCP:%s -sTCP:LISTEN.\n' "$dev_port" "$dev_port" >&2
	exit 1
}

mkdir -p "$log_dir"
: > "$log_file"

echo "Writing Tauri dev log to $log_file"
cd "$repo_root"
reclaim_dev_port

set +e
bun run tauri dev 2>&1 | tee "$log_file"
status="${PIPESTATUS[0]}"
set -e
exit "$status"
