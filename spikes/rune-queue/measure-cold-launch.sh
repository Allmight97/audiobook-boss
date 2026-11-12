#!/usr/bin/env bash
# Cold launch of the Rune batch queue and the current Tauri app.
# From the repository root, after both release binaries exist:
#
#   cargo build --release --manifest-path spikes/rune-queue/Cargo.toml
#   bun run build
#   cargo build --release -p audiobook-boss --features bundled-ffmpeg
#   bash spikes/rune-queue/measure-cold-launch.sh
#
# Prints one block per run. Linux and macOS use the same script.
# The Rune line is this machine's platform name. A Mac run is still required
# for the macOS numbers; this script does not invent them.
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
runs="${RUNS:-5}"
rune_bin="${RUNE_BIN:-}"
tauri_bin="${TAURI_BIN:-}"

if [[ -z "$rune_bin" ]]; then
	if [[ -x "$root/spikes/rune-queue/target/release/abb-rune-spike" ]]; then
		rune_bin="$root/spikes/rune-queue/target/release/abb-rune-spike"
	elif [[ -x /tmp/abb-rune-target/release/abb-rune-spike ]]; then
		rune_bin="/tmp/abb-rune-target/release/abb-rune-spike"
	fi
fi
if [[ -z "$tauri_bin" ]]; then
	if [[ -x "$root/target/release/audiobook-boss" ]]; then
		tauri_bin="$root/target/release/audiobook-boss"
	fi
fi

python3 - "$root" "$runs" "$rune_bin" "$tauri_bin" <<'PY'
import os, signal, subprocess, sys, time

root, runs, rune_bin, tauri_bin = sys.argv[1:]
runs = int(runs)
platform = "macos" if sys.platform == "darwin" else "linux" if sys.platform.startswith("linux") else sys.platform

def window_mapped():
    if sys.platform == "darwin":
        script = 'tell application "System Events" to get name of every process'
        result = subprocess.run(["osascript", "-e", script], capture_output=True, text=True)
        return "audiobook" in result.stdout.lower()
    result = subprocess.run(
        ["xdotool", "search", "--name", "AudioBook Boss"],
        capture_output=True,
        text=True,
    )
    return result.returncode == 0 and bool(result.stdout.strip())

def time_rune(binary):
    start = time.perf_counter()
    proc = subprocess.Popen(
        [binary, "--launch-probe"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    line = ""
    if proc.stdout is not None:
        line = proc.stdout.readline().rstrip("\n")
    elapsed = (time.perf_counter() - start) * 1000.0
    code = proc.wait(timeout=180)
    err = ""
    if proc.stderr is not None:
        err = proc.stderr.read()
    return elapsed, line, code, err

def time_tauri(binary):
    start = time.perf_counter()
    proc = subprocess.Popen(
        [binary],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )
    mapped = None
    deadline = start + 90.0
    while time.perf_counter() < deadline:
        if window_mapped():
            mapped = (time.perf_counter() - start) * 1000.0
            break
        if proc.poll() is not None:
            break
        time.sleep(0.05)
    os.killpg(proc.pid, signal.SIGKILL)
    try:
        proc.wait(timeout=10)
    except subprocess.TimeoutExpired:
        proc.kill()
    return mapped

print(f"platform={platform}")
print(f"runs={runs}")
if not rune_bin:
    print("rune=missing")
else:
    print(f"rune_bin={rune_bin}")
    for index in range(1, runs + 1):
        elapsed, line, code, err = time_rune(rune_bin)
        print(f"rune_run={index} shell_exec_to_line_ms={elapsed:.1f} exit={code} {line}")
        if code != 0 and err.strip():
            print("rune_stderr_begin")
            print(err.rstrip())
            print("rune_stderr_end")

if not tauri_bin:
    print("tauri=missing")
else:
    print(f"tauri_bin={tauri_bin}")
    print("tauri_phase=window_title_mapped")
    for index in range(1, runs + 1):
        mapped = time_tauri(tauri_bin)
        if mapped is None:
            print(f"tauri_run={index} window_mapped_ms=timeout")
        else:
            print(f"tauri_run={index} window_mapped_ms={mapped:.1f}")
PY
