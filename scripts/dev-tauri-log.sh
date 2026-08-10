#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
log_dir="$repo_root/.logs"
dev_port="1420"
run_id="$(date -u +"%Y%m%dT%H%M%SZ")-$$"
runs_dir="$log_dir/runs"
run_dir="$runs_dir/$run_id"
log_file="$run_dir/tauri-dev.log"
summary_file="$run_dir/tauri-dev-summary.md"
encoding_log="$run_dir/encoding.log"
latest_log_file="$log_dir/tauri-dev.log"
latest_summary_file="$log_dir/tauri-dev-summary.md"
latest_encoding_log="$log_dir/encoding.log"
retained_run_count=5
start_epoch="$(date +%s)"
start_iso="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
summary_written="false"
rust_log_source="unset"
tee_drain_failed="false"
tee_pid=""
tee_status_file="$run_dir/tee-exit-status"
orig_stdout_fd=""
orig_stderr_fd=""
declare -a port_notes=()

timestamp_utc() {
	date -u +"%Y-%m-%dT%H:%M:%SZ"
}

note_port_action() {
	port_notes+=("$*")
	printf '%s\n' "$*"
}

command_or_unavailable() {
	local output

	if output="$("$@" 2>/dev/null)"; then
		printf '%s' "${output:-ok}"
	else
		printf 'unavailable'
	fi
}

git_dirty_summary() {
	local status

	status="$(git status --short 2>/dev/null || true)"
	if [[ -n "$status" ]]; then
		printf '%s\n' "$status"
	else
		printf 'clean\n'
	fi
}

package_declared_version() {
	local package_name="$1"
	node -e 'const fs = require("fs"); const pkgName = process.argv[1]; const pkg = JSON.parse(fs.readFileSync("package.json", "utf8")); console.log(pkg.dependencies?.[pkgName] ?? pkg.devDependencies?.[pkgName] ?? pkg.peerDependencies?.[pkgName] ?? "not declared");' "$package_name" 2>/dev/null || printf 'unavailable'
}

package_installed_version() {
	local package_name="$1"
	node -e 'const fs = require("fs"); const pkgName = process.argv[1]; const path = `node_modules/${pkgName}/package.json`; if (!fs.existsSync(path)) { console.log("not installed"); process.exit(0); } console.log(JSON.parse(fs.readFileSync(path, "utf8")).version ?? "unknown");' "$package_name" 2>/dev/null || printf 'unavailable'
}

cargo_lock_version() {
	local crate_name="$1"
	awk -v target="$crate_name" '
		$0 == "[[package]]" {
			if (name == target) {
				print version
				printed = 1
				exit
			}
			name = ""
			version = ""
			next
		}
		/^name = / {
			name = $0
			sub(/^name = "/, "", name)
			sub(/"$/, "", name)
		}
		/^version = / {
			version = $0
			sub(/^version = "/, "", version)
			sub(/"$/, "", version)
		}
		END {
			if (!printed && name == target) {
				print version
				printed = 1
			}
			if (!printed) {
				print "not locked"
			}
		}
	' Cargo.lock 2>/dev/null || printf 'unavailable'
}

print_version_matrix() {
	local package_name crate_name
	local -a js_packages=(
		"@tauri-apps/api"
		"@tauri-apps/cli"
		"@tauri-apps/plugin-dialog"
		"@tauri-apps/plugin-opener"
	)
	local -a rust_crates=(
		"tauri"
		"tauri-build"
		"tauri-utils"
		"tauri-runtime"
		"tauri-runtime-wry"
		"wry"
		"tao"
		"tauri-plugin-dialog"
		"tauri-plugin-opener"
	)

	printf 'Bun: %s\n' "$(command_or_unavailable bun --version)"
	printf 'Node: %s\n' "$(command_or_unavailable node --version)"
	printf 'Rust: %s\n' "$(command_or_unavailable rustc --version)"
	printf 'Cargo: %s\n' "$(command_or_unavailable cargo --version)"
	printf '\nJS Tauri packages:\n'
	for package_name in "${js_packages[@]}"; do
		printf -- '- %s declared=%s installed=%s\n' \
			"$package_name" \
			"$(package_declared_version "$package_name")" \
			"$(package_installed_version "$package_name")"
	done
	printf '\nCargo.lock Tauri crates:\n'
	for crate_name in "${rust_crates[@]}"; do
		printf -- '- %s %s\n' "$crate_name" "$(cargo_lock_version "$crate_name")"
	done
}

print_process_snapshot() {
	ps -axo pid,ppid,stat,command 2>/dev/null | awk '
		$4 == "awk" { next }
		NR == 1 ||
		/bun run tauri dev/ ||
		/node_modules\/.bin\/tauri dev/ ||
		/target\/debug\/audiobook-boss/ ||
		/node_modules\/.bin\/vite/ ||
		/[[:space:]]vite[[:space:]]/
	'
}

write_summary() {
	local status="$1"
	local end_epoch duration end_iso
	end_epoch="$(date +%s)"
	duration="$((end_epoch - start_epoch))"
	end_iso="$(timestamp_utc)"

	{
		printf '# Tauri Dev Run Summary\n\n'
		printf -- '- Run ID: `%s`\n' "$run_id"
		printf -- '- Started: `%s`\n' "$start_iso"
		printf -- '- Ended: `%s`\n' "$end_iso"
		printf -- '- Duration: `%ss`\n' "$duration"
		printf -- '- Exit status: `%s`\n' "$status"
		printf -- '- Run directory: `%s`\n' "$run_dir"
		printf -- '- Main log: `%s`\n' "$log_file"
		printf -- '- Encoder log: `%s`\n' "$encoding_log"
		printf -- '- Latest summary entrypoint: `%s`\n' "$latest_summary_file"
		printf '\n## Git\n\n'
		printf -- '- Branch: `%s`\n' "$(command_or_unavailable git branch --show-current)"
		printf -- '- HEAD: `%s`\n' "$(command_or_unavailable git rev-parse --short HEAD)"
		printf '\n```text\n'
		git_dirty_summary
		printf '```\n'
		printf '\n## Runtime Matrix\n\n```text\n'
		print_version_matrix
		printf '```\n'
		printf '\n## Environment\n\n'
		printf -- '- RUST_LOG: `%s`\n' "${RUST_LOG:-unset}"
		printf -- '- RUST_LOG source: `%s`\n' "${rust_log_source:-unset}"
		printf -- '- ABB_RUN_ID: `%s`\n' "${ABB_RUN_ID:-unset}"
		printf -- '- ABB_ENCODING_LOG: `%s`\n' "${ABB_ENCODING_LOG:-unset}"
		printf '\n## Port Handling\n\n'
		if ((${#port_notes[@]} > 0)); then
			printf '```text\n'
			printf '%s\n' "${port_notes[@]}"
			printf '```\n'
		else
			printf 'No pre-existing ABB dev server was replaced.\n'
		fi
		printf '\n## Process Snapshot At Exit\n\n```text\n'
		print_process_snapshot
		printf '```\n'
		printf '\n'
		if [[ "$tee_drain_failed" == "true" ]]; then
			printf '## Session Verdict\n\n'
			printf -- '- Health: `indeterminate`\n'
			printf -- '- Log capture did not drain cleanly; inspect the raw run log.\n'
		elif ! bun "$repo_root/scripts/dev-log-analysis.ts" \
			--main-log "$log_file" \
			--encoding-log "$encoding_log" \
			--exit-status "$status"; then
			printf '## Session Verdict\n\n'
			printf -- '- Health: `indeterminate`\n'
			printf -- '- Dev-log analysis failed; inspect the raw run log.\n'
		fi
	} > "$summary_file"

	summary_written="true"
}

finish_run() {
	local status="$?"
	set +e
	if [[ -n "${tee_pid:-}" ]]; then
		exec >&"$orig_stdout_fd" 2>&"$orig_stderr_fd"
		# Bounded drain: an orphaned child still holding the pipe write end
		# must degrade the verdict, not hang the exit trap.
		for _ in {1..100}; do
			kill -0 "$tee_pid" 2>/dev/null || break
			sleep 0.1
		done
		# Later process substitutions make the pid unwaitable in bash, so
		# liveness plus the exit-status sidecar is the drain signal: a tee
		# that died from a write error must not pass as a clean drain.
		if kill -0 "$tee_pid" 2>/dev/null; then
			tee_drain_failed="true"
		elif [[ "$(cat "$tee_status_file" 2>/dev/null)" != "0" ]]; then
			tee_drain_failed="true"
		fi
	fi
	if [[ "$summary_written" != "true" ]]; then
		write_summary "$status"
	fi
	printf '\n=== ABB Tauri dev log finished ===\n'
	printf 'Run ID: %s\n' "$run_id"
	printf 'Ended: %s\n' "$(timestamp_utc)"
	printf 'Exit status: %s\n' "$status"
	printf 'Summary: %s\n' "$summary_file"
	printf 'Latest summary: %s\n' "$latest_summary_file"
	printf 'Encoder log: %s\n' "$encoding_log"
	return "$status"
}

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
			note_port_action "Port $dev_port is already in use by a non-ABB process; not killing it."
			note_port_action "PID: $pid"
			note_port_action "CWD: ${cwd:-unknown}"
			note_port_action "Command: ${command:-unknown}"
			note_port_action "Stop that process or free port $dev_port, then rerun bun run app:dev:log."
			exit 1
		fi
		collect_repo_dev_processes_from_pid "$pid"
	done

	note_port_action "Port $dev_port is already held by ABB dev process(es): ${dev_process_pids[*]}. Replacing them."
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

archive_legacy_latest_run() {
	local existing_run_id legacy_run_dir fallback_dir source target
	local has_regular="false"
	local -a entrypoints=("$latest_log_file" "$latest_summary_file" "$latest_encoding_log")

	for source in "${entrypoints[@]}"; do
		if [[ -f "$source" && ! -L "$source" ]]; then
			has_regular="true"
			break
		fi
	done
	[[ "$has_regular" == "true" ]] || return 0

	if [[ -f "$latest_log_file" && ! -L "$latest_log_file" ]]; then
		existing_run_id="$(sed -n 's/^Run ID: //p' "$latest_log_file" | head -n 1)"
	else
		existing_run_id=""
	fi

	if [[ "$existing_run_id" =~ ^[0-9]{8}T[0-9]{6}Z-[0-9]+$ && "$existing_run_id" != "$run_id" ]]; then
		legacy_run_dir="$runs_dir/$existing_run_id"
		mkdir -p "$legacy_run_dir"
		for source in "${entrypoints[@]}"; do
			[[ -f "$source" && ! -L "$source" ]] || continue
			target="$legacy_run_dir/$(basename "$source")"
			[[ -e "$target" ]] || cp -p "$source" "$target"
		done
		return 0
	fi

	fallback_dir="$runs_dir/legacy-${start_epoch}-$$"
	mkdir -p "$fallback_dir"
	for source in "${entrypoints[@]}"; do
		[[ -f "$source" && ! -L "$source" ]] || continue
		mv "$source" "$fallback_dir/$(basename "$source")"
	done
}

prune_run_history() {
	local candidate candidate_name remove_count index
	local -a run_dirs=()

	shopt -s nullglob
	for candidate in "$runs_dir"/*; do
		[[ -d "$candidate" ]] || continue
		candidate_name="$(basename "$candidate")"
		[[ "$candidate_name" =~ ^[0-9]{8}T[0-9]{6}Z-[0-9]+$ ]] || continue
		run_dirs+=("$candidate")
	done
	shopt -u nullglob

	remove_count=$((${#run_dirs[@]} - retained_run_count))
	((remove_count > 0)) || return 0
	for ((index = 0; index < remove_count; index++)); do
		candidate="${run_dirs[$index]}"
		[[ "$candidate" != "$run_dir" && "$candidate" == "$runs_dir/"* ]] || continue
		rm -rf -- "$candidate"
	done
}

mkdir -p "$runs_dir"
archive_legacy_latest_run
mkdir -p "$run_dir"
: > "$log_file"
: > "$summary_file"
: > "$encoding_log"

ln -sfn "runs/$run_id/tauri-dev.log" "$latest_log_file"
ln -sfn "runs/$run_id/tauri-dev-summary.md" "$latest_summary_file"
ln -sfn "runs/$run_id/encoding.log" "$latest_encoding_log"
prune_run_history

cat > "$encoding_log" <<EOF
# ABB encoding log
run_id=$run_id
started=$start_iso
main_log=$log_file

EOF

exec {orig_stdout_fd}>&1 {orig_stderr_fd}>&2
exec > >(
	tee -a "$log_file"
	printf '%s\n' "$?" > "$tee_status_file"
) 2>&1
tee_pid=$!
trap finish_run EXIT

cd "$repo_root"
export ABB_RUN_ID="$run_id"
export ABB_ENCODING_LOG="$encoding_log"
default_rust_log="audiobook_boss_lib=info,tauri=warn,wry=warn"
if [[ -n "${ABB_DEV_RUST_LOG:-}" ]]; then
	export RUST_LOG="$ABB_DEV_RUST_LOG"
	rust_log_source="ABB_DEV_RUST_LOG"
elif [[ "${ABB_DEV_USE_EXISTING_RUST_LOG:-}" == "1" && -n "${RUST_LOG:-}" ]]; then
	rust_log_source="RUST_LOG"
else
	export RUST_LOG="$default_rust_log"
	rust_log_source="dev-log default"
fi

printf '=== ABB Tauri dev log ===\n'
printf 'Run ID: %s\n' "$run_id"
printf 'Started: %s\n' "$start_iso"
printf 'Repo: %s\n' "$repo_root"
printf 'Run directory: %s\n' "$run_dir"
printf 'Main log: %s (latest: %s)\n' "$log_file" "$latest_log_file"
printf 'Summary: %s (latest: %s)\n' "$summary_file" "$latest_summary_file"
printf 'Encoder log: %s (latest: %s)\n' "$encoding_log" "$latest_encoding_log"
printf 'RUST_LOG: %s\n' "$RUST_LOG"
printf 'RUST_LOG source: %s\n' "$rust_log_source"
printf '\n=== Git ===\n'
printf 'Branch: %s\n' "$(command_or_unavailable git branch --show-current)"
printf 'HEAD: %s\n' "$(command_or_unavailable git rev-parse --short HEAD)"
printf 'Dirty files:\n'
git_dirty_summary
printf '\n=== Runtime matrix ===\n'
print_version_matrix
printf '\n=== Port preflight ===\n'
reclaim_dev_port
if ((${#port_notes[@]} == 0)); then
	printf 'Port %s is available.\n' "$dev_port"
fi
printf '\n=== Process snapshot before launch ===\n'
print_process_snapshot
printf '\n=== Tauri output ===\n'

set +e
bun run tauri dev --features bundled-ffmpeg
status="$?"
set -e
exit "$status"
