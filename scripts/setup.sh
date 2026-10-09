#!/usr/bin/env bash
set -euo pipefail

# Agent and CI setup for ABB. Idempotent. Never edits shell profiles.
#   bash scripts/setup.sh                 # frontend and rust
#   bash scripts/setup.sh frontend        # locked Bun, Node, frozen install, actionlint, shellcheck, uv, zizmor
#   bash scripts/setup.sh rust            # bundled FFmpeg build deps, FFmpeg 9 CLI, .NET SDK, helper, uv, cargo fetch
#   bash scripts/setup.sh --check [mode]  # install nothing; exit nonzero if something is missing
#   bash scripts/setup.sh --print-env     # eval-able PATH and toolchain exports
#
# rust installs nasm and static libopus for the engine's bundled FFmpeg
# (cargo compiles the vendored revision on first use and caches it in target/),
# Tauri's GTK/WebKit packages on Linux, the pinned .NET SDK and a real AAXClean
# helper for this host, and an FFmpeg 9 command-line build for media-lane fixture readback.

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tools_bin="${ABB_TOOLS_BIN:-$HOME/.local/bin}"
export BUN_INSTALL="${BUN_INSTALL:-$HOME/.bun}"
export DOTNET_ROOT="${DOTNET_ROOT:-$HOME/.dotnet}"
export DOTNET_CLI_TELEMETRY_OPTOUT=1
export DOTNET_NOLOGO=1
cargo_bin="${CARGO_HOME:-$HOME/.cargo}/bin"
export PATH="${BUN_INSTALL}/bin:${tools_bin}:${DOTNET_ROOT}:${HOME}/.cargo/bin:${cargo_bin}:${PATH}"

check_only=0
print_env_only=0
mode="all"

usage() {
	sed -n '4,14p' "$0" | sed -E 's/^# ?//'
}

while [[ $# -gt 0 ]]; do
	case "$1" in
		--check) check_only=1 ;;
		--print-env) print_env_only=1 ;;
		frontend | rust)
			if [[ "${mode}" != all ]]; then
				printf 'error: pass one mode (frontend or rust), or none for both\n' >&2
				exit 1
			fi
			mode="$1"
			;;
		-h | --help)
			usage
			exit 0
			;;
		*)
			printf 'error: unknown argument %s (expected frontend, rust, --check, or --print-env)\n' "$1" >&2
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

# Download a release asset and check it against the release's checksum file.
fetch_verified() {
	local base="$1" asset="$2" sums="$3" dest="$4"
	local expected actual
	# Bounded: a stalled download fails in minutes instead of holding CI.
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

pinned_dotnet_sdk() {
	local file="${repo_root}/tools/abb-aaxclean-helper/global.json" version
	if [[ ! -f "${file}" ]]; then
		printf 'error: missing %s\n' "${file}" >&2
		exit 1
	fi
	version="$(sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "${file}" | head -n1)"
	if [[ ! "${version}" =~ ^10\.[0-9]+\.[0-9]+$ ]]; then
		printf 'error: %s must pin .NET 10.x.x, got %s\n' "${file}" "${version}" >&2
		exit 1
	fi
	printf '%s\n' "${version}"
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

node_is_supported() {
	local major
	have node || return 1
	major="$(node_major node)"
	[[ "${major}" =~ ^[0-9]+$ && "${major}" -ge "$(pinned_node_major)" ]]
}

# Capture stdout and stderr: Homebrew prints `-version` on stderr.
# Parse with awk, not sed `\?`: macOS BSD sed left Homebrew's `version 9.0.1`
# unmatched after CI poured ffmpeg 9.0.1_1.
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

ffmpeg_bin() {
	local candidate
	candidate="${tools_bin}/ffmpeg"
	if [[ -x "${candidate}" && "$(ffmpeg_major "${candidate}")" == 9 ]]; then
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
	if [[ -x "${candidate}" && "$(ffmpeg_major "${candidate}")" == 9 ]]; then
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
	if node_is_supported; then
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
}

check_rust() {
	local ffprobe_path ffmpeg_path sidecar required_sdk sidecar_bytes required_rust rustc_ver
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
		ok "ffmpeg 9 (${ffmpeg_path})"
	else
		need "ffmpeg 9 CLI (run: bash scripts/setup.sh rust)"
	fi
	if ffprobe_path="$(ffprobe_bin)"; then
		ok "ffprobe 9 (${ffprobe_path})"
	else
		need "ffprobe 9 CLI (run: bash scripts/setup.sh rust)"
	fi
	if is_linux; then
		if have pkg-config && pkg-config --exists webkit2gtk-4.1 gtk+-3.0; then
			ok "GTK/WebKit (webkit2gtk-4.1)"
		else
			need "GTK/WebKit (run: bash scripts/setup.sh rust)"
		fi
		if have pkg-config && pkg-config --exists openssl; then
			ok "openssl ($(pkg-config --modversion openssl))"
		else
			need "openssl (run: bash scripts/setup.sh rust)"
		fi
	fi
	required_sdk="$(pinned_dotnet_sdk)"
	if [[ -x "${DOTNET_ROOT}/dotnet" ]] && "${DOTNET_ROOT}/dotnet" --list-sdks 2>/dev/null | grep -q "^${required_sdk}"; then
		ok ".NET SDK ${required_sdk}"
	elif have dotnet && dotnet --list-sdks 2>/dev/null | grep -q "^${required_sdk}"; then
		ok ".NET SDK ${required_sdk}"
	else
		need ".NET SDK ${required_sdk} (run: bash scripts/setup.sh rust)"
	fi
	if have rustc; then
		sidecar="${repo_root}/src-tauri/binaries/abb-aaxclean-helper-$(rustc -vV | awk '/^host:/ { print $2 }')"
		if [[ -x "${sidecar}" ]]; then
			sidecar_bytes="$(wc -c <"${sidecar}" | tr -d ' ')"
			if [[ "${sidecar_bytes}" -gt 1000000 ]]; then
				ok "AAXClean helper (${sidecar_bytes} bytes)"
			else
				need "AAXClean helper (found ${sidecar_bytes}-byte stub; run: bash scripts/setup.sh rust)"
			fi
		else
			need "AAXClean helper (run: bash scripts/setup.sh rust)"
		fi
	fi
}

# Eval-able PATH and toolchain exports. Host env for media tests includes
# ABB_FFMPEG / ABB_FFPROBE when setup.sh can see FFmpeg 9 (Homebrew prefix
# or ~/.local/bin), so verify.sh does not keep a second copy of that lookup.
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
	printf 'export DOTNET_ROOT=%q\n' "${DOTNET_ROOT}"
	printf 'export DOTNET_CLI_TELEMETRY_OPTOUT=1\n'
	printf 'export DOTNET_NOLOGO=1\n'
	printf 'export PATH=%q\n' "${PATH}"
}

print_path_line() {
	local brew_ffmpeg_bin rustup_bin="${HOME}/.cargo/bin"
	printf '\nAdd to PATH (this script never edits shell profiles):\n'
	printf '  eval "$(scripts/setup.sh --print-env)"\n'
	if [[ -n "${GITHUB_PATH:-}" ]]; then
		printf '%s\n' "${BUN_INSTALL}/bin" "${tools_bin}" "${DOTNET_ROOT}" >>"${GITHUB_PATH}"
		if [[ -x "${rustup_bin}/rustup" || -x "${rustup_bin}/rustc" ]]; then
			printf '%s\n' "${rustup_bin}" >>"${GITHUB_PATH}"
		fi
		if brew_ffmpeg_bin="$(brew_ffmpeg_cli ffmpeg)"; then
			printf '%s\n' "$(dirname "${brew_ffmpeg_bin}")" >>"${GITHUB_PATH}"
		fi
	fi
}

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
	print_path_line
	if [[ "${missing}" -ne 0 ]]; then
		if want_frontend && want_rust; then
			printf '\nInstall with: bash scripts/setup.sh\n' >&2
		elif want_frontend; then
			printf '\nInstall with: bash scripts/setup.sh frontend\n' >&2
		else
			printf '\nInstall with: bash scripts/setup.sh rust\n' >&2
		fi
		exit 1
	fi
	exit 0
fi

# Any Node at or above the pinned major already on PATH is kept: tools_bin is usually on the owner's
# login PATH, so installing there would replace their node in every shell.
ensure_node() {
	local required asset tmp prefix
	required="$(pinned_node_version)"
	if node_is_supported; then
		log "Using Node $(node --version)"
		return
	fi
	case "$(uname -s)-$(uname -m)" in
		Linux-x86_64) asset="node-v${required}-linux-x64.tar.xz" ;;
		Linux-aarch64 | Linux-arm64) asset="node-v${required}-linux-arm64.tar.xz" ;;
		Darwin-x86_64) asset="node-v${required}-darwin-x64.tar.xz" ;;
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
		Darwin-x86_64) asset=bun-darwin-x64 ;;
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
	# Retries and timeouts make a stalled mirror fail with a message.
	local apt_opts=(-o Acquire::Retries=3 -o Acquire::http::Timeout=30 -o Acquire::https::Timeout=30)
	run_as_root apt-get "${apt_opts[@]}" update -q
	run_as_root env DEBIAN_FRONTEND=noninteractive apt-get "${apt_opts[@]}" install -y -q --no-install-recommends "$@"
}

# Node, Bun, and the FFmpeg CLI download with curl and unpack .tar.xz and .zip.
# A minimal Ubuntu image has none of these, and frontend setup runs first.
ensure_download_tools() {
	is_linux || return 0
	if have curl && have unzip && have xz && [[ -s /etc/ssl/certs/ca-certificates.crt ]]; then
		return
	fi
	log "Installing download tools"
	apt_install ca-certificates curl unzip xz-utils
}

install_linux_packages() {
	log "Installing packages for the bundled FFmpeg build and Tauri host"
	apt_install build-essential clang nasm pkg-config libopus-dev \
		libgtk-3-dev libwebkit2gtk-4.1-dev libsoup-3.0-dev librsvg2-dev \
		libssl-dev shellcheck
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
		brew_pkgs opus pkg-config nasm ffmpeg shellcheck actionlint
	else
		printf 'error: setup.sh supports Linux and macOS\n' >&2
		exit 1
	fi
}

# Media-lane fixtures and readback spawn ffmpeg/ffprobe; distro FFmpeg 6.x
# decodes edit lists and Opus pre-skip differently than the engine's FFmpeg 9.
ensure_readback_cli() {
	if ffmpeg_bin >/dev/null && ffprobe_bin >/dev/null; then
		log "Using FFmpeg 9 readback CLI"
		return
	fi
	if is_darwin; then
		local brew_ffmpeg brew_ffprobe
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
	local asset tmp
	case "$(uname -m)" in
		x86_64) asset="ffmpeg-n9.0-latest-linux64-gpl-9.0.tar.xz" ;;
		aarch64 | arm64) asset="ffmpeg-n9.0-latest-linuxarm64-gpl-9.0.tar.xz" ;;
		*)
			printf 'error: no FFmpeg CLI build for CPU %s\n' "$(uname -m)" >&2
			exit 1
			;;
	esac
	tmp="$(mktemp -d)"
	log "Installing FFmpeg 9 readback CLI into ${tools_bin}"
	fetch_verified https://github.com/BtbN/FFmpeg-Builds/releases/download/latest \
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
		# Only the tooling lane needs shellcheck; a frontend-only agent
		# without root (Claude cloud hook) must still finish setup.
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
		log "Using $(uvx --version)"
		return
	fi
	local triple asset tmp
	case "$(uname -s)-$(uname -m)" in
		Linux-x86_64) triple=x86_64-unknown-linux-gnu ;;
		Linux-aarch64 | Linux-arm64) triple=aarch64-unknown-linux-gnu ;;
		Darwin-x86_64) triple=x86_64-apple-darwin ;;
		Darwin-arm64)
			# uv's macOS arm64 tarball uses the Rust host triple. Split so
			# the old helper sidecar name is not a literal in this file.
			triple="aarch64-"
			triple+="apple-darwin"
			;;
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

ensure_dotnet() {
	local required tmp
	required="$(pinned_dotnet_sdk)"
	if [[ -x "${DOTNET_ROOT}/dotnet" ]] && "${DOTNET_ROOT}/dotnet" --list-sdks 2>/dev/null | grep -q "^${required}"; then
		log "Using .NET SDK ${required}"
		return
	fi
	tmp="$(mktemp -d)"
	log "Installing .NET SDK ${required} into ${DOTNET_ROOT}"
	# Bounded: a stalled download fails in minutes instead of holding CI.
	local curl_opts=(-fsSL --connect-timeout 20 --max-time 600 --retry 3 --retry-all-errors)
	curl "${curl_opts[@]}" -o "${tmp}/dotnet-install.sh" https://dot.net/v1/dotnet-install.sh
	bash "${tmp}/dotnet-install.sh" --version "${required}" --install-dir "${DOTNET_ROOT}"
	rm -rf "${tmp}"
	hash -r 2>/dev/null || true
	if ! "${DOTNET_ROOT}/dotnet" --list-sdks 2>/dev/null | grep -q "^${required}"; then
		printf 'error: .NET SDK %s is not installed in %s\n' "${required}" "${DOTNET_ROOT}" >&2
		exit 1
	fi
}

ensure_aaxclean_helper() {
	ensure_bun
	ensure_dotnet
	log "Publishing AAXClean helper for this host"
	(cd "${repo_root}" && bun run aaxclean-helper:publish)
}

# Fail before minutes of installs when a prerequisite setup cannot install.
require_prerequisites() {
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
	ensure_aaxclean_helper
	ensure_uv
	# So verify.sh core still works if the network drops after setup (Codex cloud).
	log "Prefetching lizard for the complexity check"
	bash "${repo_root}/scripts/check-rust-complexity.sh" --prefetch
	log "Fetching Cargo crates"
	(cd "${repo_root}" && cargo fetch --locked)
fi

print_path_line
