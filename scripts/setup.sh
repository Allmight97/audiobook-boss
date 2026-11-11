#!/usr/bin/env bash
set -euo pipefail

# Agent and CI setup for ABB. Idempotent. Never edits shell profiles; sets
# this repo's core.hooksPath so commits run .githooks/pre-commit, unless a
# Cursor wrapper already points .cursor-original-hooks-path at those hooks.
#   bash scripts/setup.sh                 # frontend, rust, and supply-chain
#   bash scripts/setup.sh frontend        # locked Bun, Node, frozen install, actionlint, shellcheck, uv, zizmor
#   bash scripts/setup.sh rust            # bundled FFmpeg build deps and source, pinned FFmpeg readback CLI, uv, cargo fetch
#   bash scripts/setup.sh supply-chain    # pinned cargo-audit and cargo-deny
#   bash scripts/setup.sh --check [mode]  # install nothing; exit nonzero if something is missing
#                                         # (no mode checks frontend and rust, not supply-chain)
#   bash scripts/setup.sh --print-env     # eval-able PATH and toolchain exports
#
# rust installs nasm and static libopus for the engine's bundled FFmpeg
# (cargo compiles the vendored revision on first use and caches it in target/),
# Tauri's GTK/WebKit packages on Linux, and the pinned FFmpeg command-line build
# for media-lane fixture readback.

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tools_bin="${ABB_TOOLS_BIN:-$HOME/.local/share/audiobook-boss/bin}"
export BUN_INSTALL="${BUN_INSTALL:-$HOME/.bun}"
cargo_bin="${CARGO_HOME:-$HOME/.cargo}/bin"
export PATH="${BUN_INSTALL}/bin:${tools_bin}:${HOME}/.cargo/bin:${cargo_bin}:${PATH}"

check_only=0
print_env_only=0
mode="all"

usage() {
	awk 'NR > 3 && /^#/ { sub(/^# ?/, ""); print; next } NR > 3 { exit }' "$0"
}

while [[ $# -gt 0 ]]; do
	case "$1" in
		--check) check_only=1 ;;
		--print-env) print_env_only=1 ;;
		frontend | rust | supply-chain)
			if [[ "${mode}" != all ]]; then
				printf 'error: pass one mode (frontend, rust, or supply-chain), or none for all\n' >&2
				exit 1
			fi
			mode="$1"
			;;
		-h | --help)
			usage
			exit 0
			;;
		*)
			printf 'error: unknown argument %s (expected frontend, rust, supply-chain, --check, or --print-env)\n' "$1" >&2
			exit 1
			;;
	esac
	shift
done

log() {
	printf '\n==> %s\n' "$*"
}

have() {
	command -v "$1" >/dev/null 2>&1
}

os_name() {
	uname -s
}

is_linux() {
	[[ "$(os_name)" == Linux ]]
}

is_darwin() {
	[[ "$(os_name)" == Darwin ]]
}

can_run_as_root() {
	[[ "$(id -u)" -eq 0 ]] || have sudo
}

run_as_root() {
	if [[ "$(id -u)" -eq 0 ]]; then
		"$@"
	elif have sudo; then
		sudo "$@"
	else
		printf 'error: %s requires root or sudo\n' "$1" >&2
		return 1
	fi
}

sha256_of() {
	if have sha256sum; then
		sha256sum "$1" | awk '{print $1}'
	else
		shasum -a 256 "$1" | awk '{print $1}'
	fi
}

hash_from_sums() {
	local sums="$1" asset="$2"
	awk -v asset="${asset}" '
		{
			name = $2
			sub(/^\*/, "", name)
			if (name == asset) {
				print $1
				exit
			}
		}
	' "${sums}"
}

fetch_verified() {
	local base="$1" asset="$2" sums="$3" dest="$4"
	local expected actual
	local curl_opts=(-fsSL --connect-timeout 20 --max-time 600 --retry 3 --retry-all-errors)
	curl "${curl_opts[@]}" -o "${dest}/${asset}" "${base}/${asset}"
	curl "${curl_opts[@]}" -o "${dest}/${sums}" "${base}/${sums}"
	expected="$(hash_from_sums "${dest}/${sums}" "${asset}")"
	actual="$(sha256_of "${dest}/${asset}")"
	if [[ -z "${expected}" || "${expected}" != "${actual}" ]]; then
		printf 'error: checksum mismatch for %s\n' "${asset}" >&2
		exit 1
	fi
}

pinned_node_version() {
	local file="${repo_root}/.node-version" version
	if [[ ! -f "${file}" ]]; then
		printf 'error: missing %s\n' "${file}" >&2
		exit 1
	fi
	version="$(tr -d ' \t\r\nvV' <"${file}")"
	if [[ ! "${version}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
		printf 'error: %s must pin an exact Node version (x.y.z), got %s\n' "${file}" "${version}" >&2
		exit 1
	fi
	printf '%s\n' "${version}"
}

pinned_rust_channel() {
	local file="${repo_root}/rust-toolchain.toml" channel
	if [[ ! -f "${file}" ]]; then
		printf 'error: missing %s\n' "${file}" >&2
		exit 1
	fi
	channel="$(sed -n 's/^channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "${file}" | head -n1)"
	if [[ -z "${channel}" ]]; then
		printf 'error: %s has no channel\n' "${file}" >&2
		exit 1
	fi
	printf '%s\n' "${channel}"
}

node_major() {
	local version
	version="$("$@" --version 2>/dev/null | awk 'NR==1 {print; exit}')" # allow-silence: no version reads as unsupported Node, which the caller reports
	version="${version#v}"
	printf '%s\n' "${version%%.*}"
}

pinned_node_major() {
	local version
	version="$(pinned_node_version)"
	printf '%s\n' "${version%%.*}"
}

node_meets_pinned_major() {
	local major
	have node || return 1
	major="$(node_major node)"
	[[ "${major}" =~ ^[0-9]+$ && "${major}" -ge "$(pinned_node_major)" ]]
}

# Capture stdout and stderr: Homebrew prints `-version` on stderr.
# Parse with awk, not sed `\?`: BSD sed reads it as a literal.
ffmpeg_major() {
	local bin="$1"
	"${bin}" -version 2>&1 | awk '
		$2 == "version" {
			v = $3
			sub(/^n/, "", v)
			split(v, parts, ".")
			if (parts[1] ~ /^[0-9]+$/) print parts[1]
			exit
		}
	' || true
}

# Formula prefix, not PATH: Homebrew may leave ffmpeg unlinked from bin/.
brew_ffmpeg_cli() {
	local name="$1" prefix
	is_darwin || return 1
	have brew || return 1
	prefix="$(brew --prefix ffmpeg 2>/dev/null)" || return 1 # allow-silence: brew errors when ffmpeg is absent; absent is the answer
	[[ -x "${prefix}/bin/${name}" ]] || return 1
	printf '%s\n' "${prefix}/bin/${name}"
}

# BtbN appends -YYYYMMDD to the git-describe id stored in ffmpeg_cli_build.
ffmpeg_version_token() {
	local bin="$1"
	"${bin}" -version 2>&1 | awk '$2 == "version" { print $3; exit }' || true
}

ffmpeg_cli_matches_pin() {
	local token="$1"
	[[ "${token}" == "${ffmpeg_cli_build}" || "${token}" == "${ffmpeg_cli_build}-"* ]]
}

# Major 9 is Homebrew and the previous release-branch CLI. A master autobuild's
# version token is N-<count>-g<sha>, so the pinned build id counts as well.
tools_readback_ok() {
	local bin="$1"
	[[ -x "${bin}" ]] || return 1
	[[ "$(ffmpeg_major "${bin}")" == 9 ]] && return 0
	ffmpeg_cli_matches_pin "$(ffmpeg_version_token "${bin}")"
}

ffmpeg_bin() {
	local candidate
	candidate="${tools_bin}/ffmpeg"
	if tools_readback_ok "${candidate}"; then
		printf '%s\n' "${candidate}"
		return 0
	fi
	if candidate="$(brew_ffmpeg_cli ffmpeg)" && [[ "$(ffmpeg_major "${candidate}")" == 9 ]]; then
		printf '%s\n' "${candidate}"
		return 0
	fi
	if have ffmpeg && [[ "$(ffmpeg_major ffmpeg)" == 9 ]]; then
		command -v ffmpeg
		return 0
	fi
	return 1
}

ffprobe_bin() {
	local candidate
	candidate="${tools_bin}/ffprobe"
	if tools_readback_ok "${candidate}"; then
		printf '%s\n' "${candidate}"
		return 0
	fi
	if candidate="$(brew_ffmpeg_cli ffprobe)" && [[ "$(ffmpeg_major "${candidate}")" == 9 ]]; then
		printf '%s\n' "${candidate}"
		return 0
	fi
	if have ffprobe && [[ "$(ffmpeg_major ffprobe)" == 9 ]]; then
		command -v ffprobe
		return 0
	fi
	return 1
}

missing=0

ok() {
	printf 'OK      %s\n' "$*"
}

need() {
	printf 'MISSING %s\n' "$*"
	missing=1
}

want_frontend() {
	[[ "${mode}" == all || "${mode}" == frontend ]]
}

want_rust() {
	[[ "${mode}" == all || "${mode}" == rust ]]
}

want_supply_chain() {
	[[ "${mode}" == all || "${mode}" == supply-chain ]]
}

cargo_audit_version=0.22.1
cargo_deny_version=0.20.2

installed_cargo_tool_version() {
	cargo "$1" --version 2>/dev/null | awk 'NR == 1 {print $NF; exit}'
}

check_supply_chain() {
	local subcommand version
	for subcommand in audit deny; do
		version="cargo_${subcommand}_version"
		if [[ "$(installed_cargo_tool_version "${subcommand}")" == "${!version}" ]]; then
			ok "cargo-${subcommand} ${!version}"
		else
			need "cargo-${subcommand} ${!version} (run: bash scripts/setup.sh supply-chain)"
		fi
	done
}

# True when commits run this repo's .githooks. Cursor cloud wraps
# core.hooksPath and records the original path in .cursor-original-hooks-path.
git_hooks_installed() {
	local root="${1:-${repo_root}}"
	local hooks original
	hooks="$(git -C "${root}" config --local --get core.hooksPath || true)" # allow-silence: an unset key exits 1 and means not installed
	if [[ -z "${hooks}" ]]; then
		return 1
	fi
	if [[ "${hooks}" != /* ]]; then
		hooks="${root}/${hooks}"
	fi
	if [[ "${hooks}" == "${root}/.githooks" ]]; then
		return 0
	fi
	if [[ ! -f "${hooks}/.cursor-original-hooks-path" ]]; then
		return 1
	fi
	original="$(tr -d '[:space:]' <"${hooks}/.cursor-original-hooks-path")"
	if [[ "${original}" != /* ]]; then
		original="${root}/${original}"
	fi
	[[ "${original}" == "${root}/.githooks" ]]
}

check_frontend() {
	local required_bun required_node bun_ver
	required_bun="$(bash "${repo_root}/scripts/locked-bun-version.sh")"
	required_node="$(pinned_node_version)"
	if have bun; then
		bun_ver="$(bun --version)"
		if [[ "${bun_ver}" == "${required_bun}" ]]; then
			ok "bun ${bun_ver}"
		else
			need "bun ${required_bun} (found ${bun_ver})"
		fi
	else
		need "bun ${required_bun}"
	fi
	if node_meets_pinned_major; then
		ok "node $(node --version) (pin ${required_node})"
	elif have node; then
		need "node ${required_node%%.*} or newer (found $(node --version); pin ${required_node})"
	else
		need "node ${required_node}"
	fi
	if [[ -d "${repo_root}/node_modules" ]]; then
		ok "node_modules"
	else
		need "node_modules (run: bash scripts/setup.sh frontend)"
	fi
	if git_hooks_installed "${repo_root}"; then
		ok "git hooks"
	else
		need "git hooks (run: bash scripts/setup.sh frontend)"
	fi
}

linux_rust_packages=(
	build-essential clang mold nasm pkg-config libopus-dev
	libgtk-3-dev libwebkit2gtk-4.1-dev libsoup-3.0-dev librsvg2-dev
	shellcheck
)

missing_apt_packages() {
	local pkg
	for pkg in "$@"; do
		if ! dpkg-query -W -f='${Status}' "${pkg}" 2>/dev/null | grep -q 'install ok installed'; then # allow-silence: an unknown package exits nonzero and means install it
			printf '%s\n' "${pkg}"
		fi
	done
}

check_rust() {
	local ffprobe_path ffmpeg_path required_rust rustc_ver
	required_rust="$(pinned_rust_channel)"
	if have rustc && have cargo; then
		rustc_ver="$(
			cd "${repo_root}" && rustc --version | awk '{print $2}'
		)"
		if [[ "${rustc_ver}" == "${required_rust}" || "${rustc_ver}" == "${required_rust}".* ]]; then
			ok "rustc ${rustc_ver}"
		else
			need "rustc ${required_rust} (found ${rustc_ver:-none}; rust-toolchain.toml). Add \$HOME/.cargo/bin to PATH so rustup wins over a distro rustc"
		fi
	else
		need "rustc and cargo ${required_rust} (rust-toolchain.toml). Install rustup from https://rustup.rs"
	fi
	if cargo fmt --version >/dev/null 2>&1; then
		ok "rustfmt"
	else
		need "rustfmt (rust-toolchain.toml component)"
	fi
	if cargo clippy -V >/dev/null 2>&1; then
		ok "clippy"
	else
		need "clippy (rust-toolchain.toml component)"
	fi
	if have uvx; then
		ok "uvx"
	else
		need "uvx (run: bash scripts/setup.sh rust)"
	fi
	if have nasm; then
		ok "nasm"
	else
		need "nasm (run: bash scripts/setup.sh rust)"
	fi
	if have pkg-config && pkg-config --exists opus; then
		ok "libopus ($(pkg-config --modversion opus))"
	else
		need "libopus / pkg-config opus (run: bash scripts/setup.sh rust)"
	fi
	if ffmpeg_path="$(ffmpeg_bin)"; then
		ok "ffmpeg readback (${ffmpeg_path})"
	else
		need "pinned FFmpeg readback CLI (run: bash scripts/setup.sh rust)"
	fi
	if ffprobe_path="$(ffprobe_bin)"; then
		ok "ffprobe readback (${ffprobe_path})"
	else
		need "pinned FFmpeg readback CLI (run: bash scripts/setup.sh rust)"
	fi
	if is_linux; then
		local missing_packages
		mapfile -t missing_packages < <(missing_apt_packages "${linux_rust_packages[@]}") # allow-gnu: runs only inside the is_linux branch
		if [[ ${#missing_packages[@]} -eq 0 ]]; then
			ok "Linux build and Tauri packages"
		else
			need "Linux packages ${missing_packages[*]} (run as root or with sudo: bash scripts/setup.sh rust)"
		fi
	fi
}

print_env() {
	local ffmpeg ffprobe ffmpeg_dir
	if ffmpeg="$(ffmpeg_bin)"; then
		printf 'export ABB_FFMPEG=%q\n' "${ffmpeg}"
		ffmpeg_dir="$(dirname "${ffmpeg}")"
		if [[ ":${PATH}:" != *":${ffmpeg_dir}:"* ]]; then
			PATH="${ffmpeg_dir}:${PATH}"
		fi
	fi
	if ffprobe="$(ffprobe_bin)"; then
		printf 'export ABB_FFPROBE=%q\n' "${ffprobe}"
	fi
	printf 'export BUN_INSTALL=%q\n' "${BUN_INSTALL}"
	printf 'export PATH=%q\n' "${PATH}"
}

print_path_line() {
	local brew_ffmpeg_bin rustup_bin="${HOME}/.cargo/bin"
	printf '\nAdd to PATH (this script never edits shell profiles):\n'
	printf '  eval "$(scripts/setup.sh --print-env)"\n'
	if [[ -n "${GITHUB_PATH:-}" ]]; then
		printf '%s\n' "${BUN_INSTALL}/bin" "${tools_bin}" >>"${GITHUB_PATH}"
		if [[ -x "${rustup_bin}/rustup" || -x "${rustup_bin}/rustc" ]]; then
			printf '%s\n' "${rustup_bin}" >>"${GITHUB_PATH}"
		fi
		if brew_ffmpeg_bin="$(brew_ffmpeg_cli ffmpeg)"; then
			printf '%s\n' "$(dirname "${brew_ffmpeg_bin}")" >>"${GITHUB_PATH}"
		fi
	fi
}

# Linux readback is the BtbN master autobuild of the commit in ffmpeg-revision.
# Master assets are ffmpeg-<describe>-linux*-gpl.tar.xz. Distro FFmpeg 6.x
# decodes edit lists and Opus pre-skip differently, so it stays refused.
# Assigned before --print-env and --check, which both ask which binary this is.
ffmpeg_cli_release=autobuild-2026-11-11-12-39
ffmpeg_cli_build=N-128790-g9bee1e603c

if [[ "${print_env_only}" -eq 1 ]]; then
	print_env
	exit 0
fi

if [[ "${check_only}" -eq 1 ]]; then
	if want_frontend; then
		check_frontend
	fi
	if want_rust; then
		check_rust
	fi
	if [[ "${mode}" == supply-chain ]]; then
		check_supply_chain
	fi
	print_path_line
	if [[ "${missing}" -ne 0 ]]; then
		if [[ "${mode}" == all ]]; then
			printf '\nInstall with: bash scripts/setup.sh\n' >&2
		else
			printf '\nInstall with: bash scripts/setup.sh %s\n' "${mode}" >&2
		fi
		exit 1
	fi
	exit 0
fi

ensure_node() {
	local required asset tmp prefix
	required="$(pinned_node_version)"
	if node_meets_pinned_major; then
		log "Using Node $(node --version)"
		return
	fi
	case "$(uname -s)-$(uname -m)" in
		Linux-x86_64) asset="node-v${required}-linux-x64.tar.xz" ;;
		Linux-aarch64 | Linux-arm64) asset="node-v${required}-linux-arm64.tar.xz" ;;
		Darwin-arm64) asset="node-v${required}-darwin-arm64.tar.xz" ;;
		*)
			printf 'error: no Node release for %s %s\n' "$(uname -s)" "$(uname -m)" >&2
			exit 1
			;;
	esac
	tmp="$(mktemp -d)"
	log "Installing Node ${required} (${asset})"
	fetch_verified "https://nodejs.org/dist/v${required}" "${asset}" SHASUMS256.txt "${tmp}"
	tar -xJf "${tmp}/${asset}" -C "${tmp}"
	prefix="${tmp}/${asset%.tar.xz}"
	mkdir -p "${tools_bin}"
	install -m 755 "${prefix}/bin/node" "${tools_bin}/node"
	rm -rf "${tmp}"
	hash -r 2>/dev/null || true
	if [[ "$(node --version)" != "v${required}" ]]; then
		printf 'error: need Node %s; found %s\n' "${required}" "$(node --version)" >&2
		exit 1
	fi
}

ensure_bun() {
	local required_bun_version asset tmp
	required_bun_version="$(bash "${repo_root}/scripts/locked-bun-version.sh")"
	if have bun && [[ "$(bun --version)" == "${required_bun_version}" ]]; then
		log "Using Bun ${required_bun_version}"
		return
	fi

	case "$(uname -s)-$(uname -m)" in
		Linux-x86_64)
			asset=bun-linux-x64
			if [[ -r /proc/cpuinfo ]] && ! grep -q avx2 /proc/cpuinfo; then
				asset=bun-linux-x64-baseline
			fi
			;;
		Linux-aarch64 | Linux-arm64) asset=bun-linux-aarch64 ;;
		Darwin-arm64) asset=bun-darwin-aarch64 ;;
		*)
			printf 'error: no Bun release for %s %s\n' "$(uname -s)" "$(uname -m)" >&2
			exit 1
			;;
	esac

	# GitHub releases, not bun.sh: some agent network policies deny bun.sh.
	tmp="$(mktemp -d)"
	log "Installing Bun ${required_bun_version} (${asset})"
	fetch_verified "https://github.com/oven-sh/bun/releases/download/bun-v${required_bun_version}" \
		"${asset}.zip" SHASUMS256.txt "${tmp}"
	if ! have unzip; then
		printf 'error: unzip is required to install Bun\n' >&2
		exit 1
	fi
	unzip -oq "${tmp}/${asset}.zip" -d "${tmp}"
	mkdir -p "${BUN_INSTALL}/bin"
	install -m 755 "${tmp}/${asset}/bun" "${BUN_INSTALL}/bin/bun"
	rm -rf "${tmp}"
	hash -r 2>/dev/null || true
	if [[ "$(bun --version)" != "${required_bun_version}" ]]; then
		printf 'error: need Bun %s; found %s\n' "${required_bun_version}" "$(bun --version)" >&2
		exit 1
	fi
}

brew_pkgs() {
	export HOMEBREW_NO_AUTO_UPDATE=1
	export HOMEBREW_NO_ANALYTICS=1
	log "Installing Homebrew packages: $*"
	brew install "$@"
}

apt_install() {
	local apt_opts=(-o Acquire::Retries=3 -o Acquire::http::Timeout=30 -o Acquire::https::Timeout=30)
	run_as_root apt-get "${apt_opts[@]}" update -q
	run_as_root env DEBIAN_FRONTEND=noninteractive apt-get "${apt_opts[@]}" install -y -q --no-install-recommends "$@"
}

# Node, Bun, and the FFmpeg CLI download with curl and unpack .tar.xz and .zip.
# A minimal Ubuntu image has none of these, and frontend setup runs first.
ensure_download_tools() {
	is_linux || return 0
	if have curl && have unzip && have xz && [[ -s /etc/ssl/certs/ca-certificates.crt ]] &&
		/sbin/ldconfig -p | grep -q 'libatomic\.so\.1'; then
		return
	fi
	log "Installing download tools"
	apt_install ca-certificates curl unzip xz-utils libatomic1
}

install_linux_packages() {
	local missing
	mapfile -t missing < <(missing_apt_packages "${linux_rust_packages[@]}") # allow-gnu: install_linux_packages runs only on Linux
	if [[ ${#missing[@]} -eq 0 ]]; then
		return
	fi
	log "Installing packages for the bundled FFmpeg build and Tauri host: ${missing[*]}"
	apt_install "${missing[@]}"
}

install_linux_shellcheck() {
	if have shellcheck; then
		return
	fi
	log "Installing shellcheck"
	apt_install shellcheck
}

install_rust_packages() {
	if is_linux; then
		install_linux_packages
	elif is_darwin; then
		brew_pkgs opus pkg-config nasm shellcheck actionlint
	else
		printf 'error: setup.sh supports Linux and macOS\n' >&2
		exit 1
	fi
}

pinned_readback_installed() {
	local ff="${tools_bin}/ffmpeg" fp="${tools_bin}/ffprobe"
	[[ -x "${ff}" && -x "${fp}" ]] || return 1
	ffmpeg_cli_matches_pin "$(ffmpeg_version_token "${ff}")" \
		&& ffmpeg_cli_matches_pin "$(ffmpeg_version_token "${fp}")"
}

ensure_readback_cli() {
	if is_darwin; then
		local brew_ffmpeg brew_ffprobe
		if ffmpeg_bin >/dev/null && ffprobe_bin >/dev/null; then
			log "Using FFmpeg 9 readback CLI"
			return
		fi
		brew_pkgs ffmpeg
		hash -r 2>/dev/null || true
		if ffmpeg_bin >/dev/null && ffprobe_bin >/dev/null; then
			log "Using FFmpeg 9 readback CLI"
			return
		fi
		printf 'error: Homebrew ffmpeg is not major version 9\n' >&2
		printf 'command -v ffmpeg: %s\n' "$(command -v ffmpeg 2>/dev/null || printf missing)" >&2 # allow-silence: diagnostic line; "missing" is the answer
		printf 'command -v ffprobe: %s\n' "$(command -v ffprobe 2>/dev/null || printf missing)" >&2 # allow-silence: diagnostic line; "missing" is the answer
		printf 'brew --prefix ffmpeg: %s\n' "$(brew --prefix ffmpeg 2>/dev/null || printf missing)" >&2 # allow-silence: diagnostic line; "missing" is the answer
		if brew_ffmpeg="$(brew_ffmpeg_cli ffmpeg)"; then
			printf 'brew ffmpeg -version: %s\n' "$("${brew_ffmpeg}" -version 2>&1 | awk 'NR==1 {print; exit}')" >&2
		fi
		if brew_ffprobe="$(brew_ffmpeg_cli ffprobe)"; then
			printf 'brew ffprobe -version: %s\n' "$("${brew_ffprobe}" -version 2>&1 | awk 'NR==1 {print; exit}')" >&2
		fi
		if have ffmpeg; then
			printf 'PATH ffmpeg -version: %s\n' "$(ffmpeg -version 2>&1 | awk 'NR==1 {print; exit}')" >&2
		fi
		exit 1
	fi
	if pinned_readback_installed; then
		log "Using FFmpeg readback CLI ${ffmpeg_cli_build}"
		return
	fi
	local asset tmp
	case "$(uname -m)" in
		x86_64) asset="ffmpeg-${ffmpeg_cli_build}-linux64-gpl.tar.xz" ;;
		aarch64 | arm64) asset="ffmpeg-${ffmpeg_cli_build}-linuxarm64-gpl.tar.xz" ;;
		*)
			printf 'error: no FFmpeg CLI build for CPU %s\n' "$(uname -m)" >&2
			exit 1
			;;
	esac
	tmp="$(mktemp -d)"
	log "Installing FFmpeg readback CLI ${ffmpeg_cli_build} into ${tools_bin}"
	fetch_verified "https://github.com/FFmpeg-Builds/FFmpeg-Builds/releases/download/${ffmpeg_cli_release}" \
		"${asset}" checksums.sha256 "${tmp}"
	tar -xJf "${tmp}/${asset}" -C "${tmp}"
	mkdir -p "${tools_bin}"
	install -m 755 "${tmp}/${asset%.tar.xz}/bin/ffmpeg" "${tmp}/${asset%.tar.xz}/bin/ffprobe" "${tools_bin}/"
	rm -rf "${tmp}"
}

actionlint_version=1.7.12

ensure_actionlint() {
	local current=""
	if have actionlint; then
		current="$(actionlint -version | awk 'NR==1 {print; exit}')"
		if [[ "${current}" == "${actionlint_version}" ]]; then
			return
		fi
	fi
	if is_darwin; then
		brew_pkgs actionlint
		return
	fi
	local arch tmp asset
	case "$(uname -m)" in
		x86_64) arch=amd64 ;;
		aarch64 | arm64) arch=arm64 ;;
		*)
			printf 'error: no actionlint build for CPU %s\n' "$(uname -m)" >&2
			exit 1
			;;
	esac
	asset="actionlint_${actionlint_version}_linux_${arch}.tar.gz"
	tmp="$(mktemp -d)"
	log "Installing actionlint ${actionlint_version} into ${tools_bin}"
	fetch_verified "https://github.com/rhysd/actionlint/releases/download/v${actionlint_version}" \
		"${asset}" "actionlint_${actionlint_version}_checksums.txt" "${tmp}"
	tar -xzf "${tmp}/${asset}" -C "${tmp}" actionlint
	mkdir -p "${tools_bin}"
	install -m 755 "${tmp}/actionlint" "${tools_bin}/"
	rm -rf "${tmp}"
}

ensure_shellcheck() {
	if have shellcheck; then
		return
	fi
	if is_linux; then
		if ! can_run_as_root; then
			log "Skipping shellcheck: apt needs root or sudo. Only verify.sh tooling uses it."
			return
		fi
		install_linux_shellcheck
	elif is_darwin; then
		brew_pkgs shellcheck
	else
		printf 'error: no shellcheck install for %s\n' "$(os_name)" >&2
		exit 1
	fi
}

uv_version=0.12.10

ensure_uv() {
	if have uvx; then
		log "Using uv $(uvx --version | awk 'NR == 1 {print $2; exit}')"
		return
	fi
	local triple asset tmp
	case "$(uname -s)-$(uname -m)" in
		Linux-x86_64) triple=x86_64-unknown-linux-gnu ;;
		Linux-aarch64 | Linux-arm64) triple=aarch64-unknown-linux-gnu ;;
		Darwin-arm64) triple=aarch64-apple-darwin ;;
		*)
			printf 'error: no uv release for %s %s\n' "$(uname -s)" "$(uname -m)" >&2
			exit 1
			;;
	esac
	asset="uv-${triple}.tar.gz"
	tmp="$(mktemp -d)"
	log "Installing uv ${uv_version} into ${tools_bin}"
	fetch_verified "https://github.com/astral-sh/uv/releases/download/${uv_version}" \
		"${asset}" sha256.sum "${tmp}"
	tar -xzf "${tmp}/${asset}" -C "${tmp}"
	mkdir -p "${tools_bin}"
	if [[ -x "${tmp}/uv-${triple}/uv" ]]; then
		install -m 755 "${tmp}/uv-${triple}/uv" "${tmp}/uv-${triple}/uvx" "${tools_bin}/"
	else
		install -m 755 "${tmp}/uv" "${tmp}/uvx" "${tools_bin}/"
	fi
	rm -rf "${tmp}"
}

install_cargo_tool() {
	local tool="$1" version="$2" url="$3" sha256="$4" tmp
	tmp="$(mktemp -d)"
	log "Installing ${tool} ${version} into ${tools_bin}"
	curl -fsSL --connect-timeout 20 --max-time 600 --retry 3 --retry-all-errors -o "${tmp}/archive.tgz" "${url}"
	if [[ "$(sha256_of "${tmp}/archive.tgz")" != "${sha256}" ]]; then
		printf 'error: checksum mismatch for %s\n' "${url}" >&2
		exit 1
	fi
	tar -xzf "${tmp}/archive.tgz" -C "${tmp}"
	mkdir -p "${tools_bin}"
	install -m 755 "$(find "${tmp}" -type f -name "${tool}" | head -n 1)" "${tools_bin}/"
	rm -rf "${tmp}"
}

ensure_cargo_audit() {
	local triple sha256 base
	if [[ "$(installed_cargo_tool_version audit)" == "${cargo_audit_version}" ]]; then
		log "Using cargo-audit ${cargo_audit_version}"
		return
	fi
	case "$(uname -s)-$(uname -m)" in
		Linux-x86_64)
			triple=x86_64-unknown-linux-musl
			sha256=c32506f338bdcdaef5a17fb9f33abb6ecf9561324cfd34237fd335f9283a1eab
			;;
		Linux-aarch64 | Linux-arm64)
			triple=aarch64-unknown-linux-gnu
			sha256=4c8df835ee484441bd2c8c6bcac28c4ce4b4058ba9e7477cb9e0012fe7769f66
			;;
		Darwin-arm64)
			triple=aarch64-apple-darwin
			sha256=04e76e1da25f597bea4814c44faf8aac215838b9f3646e3b6a873d87acd31b73
			;;
		Darwin-x86_64)
			triple=x86_64-apple-darwin
			sha256=582d104a2a4bdb127c6bf6d056d89eede40686d11f52e4bc1765132ec99d2fca
			;;
		*)
			printf 'error: no cargo-audit release for %s %s\n' "$(uname -s)" "$(uname -m)" >&2
			exit 1
			;;
	esac
	base="https://github.com/rustsec/rustsec/releases/download/cargo-audit%2Fv${cargo_audit_version}"
	install_cargo_tool cargo-audit "${cargo_audit_version}" \
		"${base}/cargo-audit-${triple}-v${cargo_audit_version}.tgz" "${sha256}"
}

ensure_cargo_deny() {
	local triple sha256
	if [[ "$(installed_cargo_tool_version deny)" == "${cargo_deny_version}" ]]; then
		log "Using cargo-deny ${cargo_deny_version}"
		return
	fi
	case "$(uname -s)-$(uname -m)" in
		Linux-x86_64)
			triple=x86_64-unknown-linux-musl
			sha256=9f12ed4c49936e09b48bf862b595cde2fe64fcbd9d74dfacac6131ca824c8d5f
			;;
		Linux-aarch64 | Linux-arm64)
			triple=aarch64-unknown-linux-musl
			sha256=995c82be0defc7a025cae49a2aa2644ce8245c9a3318fc4103907c6a285e8c7d
			;;
		Darwin-arm64)
			triple=aarch64-apple-darwin
			sha256=fe67d82a10d8597a3549364cb733a3f9cc1bfff9031b7ae46384a9f2a72090c3
			;;
		Darwin-x86_64)
			triple=x86_64-apple-darwin
			sha256=248da7f581724e470071990c088ffc55c811981715f4cbdb258621fb79f8b7a6
			;;
		*)
			printf 'error: no cargo-deny release for %s %s\n' "$(uname -s)" "$(uname -m)" >&2
			exit 1
			;;
	esac
	install_cargo_tool cargo-deny "${cargo_deny_version}" \
		"https://github.com/EmbarkStudios/cargo-deny/releases/download/${cargo_deny_version}/cargo-deny-${cargo_deny_version}-${triple}.tar.gz" \
		"${sha256}"
}

# The bundled FFmpeg build clones its pinned commit from this mirror when it
# exists (vendor/ffmpeg-sys-next-*/build.rs, source_remote), so verify works
# offline after setup.
ensure_ffmpeg_source() {
	if [[ -n "${CI:-}" ]]; then
		log "Skipping the offline FFmpeg source: CI has the network and restores built FFmpeg trees"
		return
	fi
	local commit mirror
	commit="$(tr -d '[:space:]' <"$(echo "${repo_root}"/vendor/ffmpeg-sys-next-*/ffmpeg-revision)")"
	mirror="${CARGO_TARGET_DIR:-${repo_root}/target}/abb-ffmpeg-source/${commit}.git"
	if git -C "${mirror}" cat-file -e "${commit}^{commit}" 2>/dev/null; then # allow-silence: a missing mirror exits nonzero and means fetch it
		log "Using FFmpeg source ${commit}"
		return
	fi
	log "Fetching FFmpeg source ${commit} for offline builds"
	rm -rf "$(dirname "${mirror}")"
	git init -q --bare "${mirror}"
	git -C "${mirror}" fetch -q --depth=1 https://github.com/FFmpeg/FFmpeg "${commit}"
	git -C "${mirror}" update-ref refs/heads/pinned "${commit}"
}

install_git_hooks() {
	if ! git_hooks_installed "${repo_root}"; then
		log "Installing git hooks (core.hooksPath .githooks)"
		git -C "${repo_root}" config core.hooksPath .githooks
	fi
}

require_prerequisites() {
	if is_darwin && [[ "$(uname -m)" != arm64 ]]; then
		printf 'error: ABB supports Apple Silicon Macs only (README, Requirements).\n' >&2
		exit 1
	fi
	if is_darwin && ! have brew; then
		printf 'error: Homebrew is required on macOS. Install it from https://brew.sh, then rerun.\n' >&2
		exit 1
	fi
	if want_rust && ! have cargo; then
		printf 'error: Rust is missing. Install rustup from https://rustup.rs (it reads rust-toolchain.toml), then rerun.\n' >&2
		exit 1
	fi
}

require_prerequisites
ensure_download_tools
install_git_hooks

if want_frontend; then
	ensure_node
	ensure_bun
	log "Installing frontend dependencies"
	(cd "${repo_root}" && bun install --frozen-lockfile)
	ensure_actionlint
	ensure_shellcheck
	ensure_uv
	log "Prefetching zizmor for the workflow check"
	bash "${repo_root}/scripts/check-workflows.sh" --prefetch
fi

if want_rust; then
	install_rust_packages
	ensure_readback_cli
	ensure_bun
	ensure_uv
	# So verify.sh core still works if the network drops after setup (Codex cloud).
	log "Prefetching lizard for the complexity check"
	bash "${repo_root}/scripts/check-rust-complexity.sh" --prefetch
	log "Fetching Cargo crates"
	(cd "${repo_root}" && cargo fetch --locked)
	ensure_ffmpeg_source
fi

if want_supply_chain; then
	ensure_cargo_audit
	ensure_cargo_deny
fi

print_path_line
