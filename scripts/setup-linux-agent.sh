#!/usr/bin/env bash
set -euo pipefail

# Linux agent and CI setup for ABB. Idempotent.
#   bash scripts/setup-linux-agent.sh          # locked Bun + frozen frontend install
#   bash scripts/setup-linux-agent.sh --rust   # what Rust engine/host proof needs
#
# --rust installs nasm and static libopus for the engine's bundled FFmpeg
# (cargo compiles the vendored revision on first use and caches it in target/),
# Tauri's GTK/WebKit packages and the AAXClean sidecar stub for host builds,
# and an FFmpeg 9 command-line build for media-lane fixture readback.

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tools_bin="${ABB_TOOLS_BIN:-$HOME/.local/bin}"
mode="${1:-frontend}"

log() {
	printf '\n==> %s\n' "$*"
}

have() {
	command -v "$1" >/dev/null 2>&1
}

run_as_root() {
	if [ "$(id -u)" -eq 0 ]; then
		"$@"
	elif have sudo; then
		sudo "$@"
	else
		printf 'error: %s requires root or sudo\n' "$1" >&2
		return 1
	fi
}

# Download a release asset and check it against the release's checksum file.
fetch_verified() {
	local base="$1" asset="$2" sums="$3" dest="$4"
	curl -fsSL -o "${dest}/${asset}" "${base}/${asset}"
	curl -fsSL -o "${dest}/${sums}" "${base}/${sums}"
	(cd "${dest}" && grep " ${asset}\$" "${sums}" | sha256sum -c -)
}

ensure_bun() {
	local required_bun_version
	required_bun_version="$(bash "${repo_root}/scripts/locked-bun-version.sh")"
	if have bun && [ "$(bun --version)" = "${required_bun_version}" ]; then
		log "Using Bun ${required_bun_version}"
		return
	fi

	# Bun's x64 default build needs AVX2; the baseline build runs without it.
	local asset tmp
	case "$(uname -m)" in
		x86_64) asset=bun-linux-x64 ;;
		aarch64 | arm64) asset=bun-linux-aarch64 ;;
		*) printf 'error: no Bun release for CPU %s\n' "$(uname -m)" >&2; exit 1 ;;
	esac
	if [ "${asset}" = bun-linux-x64 ] && ! grep -q avx2 /proc/cpuinfo; then
		asset=bun-linux-x64-baseline
	fi

	# GitHub releases, not bun.sh: some agent network policies deny bun.sh.
	tmp="$(mktemp -d)"
	log "Installing Bun ${required_bun_version} (${asset})"
	fetch_verified "https://github.com/oven-sh/bun/releases/download/bun-v${required_bun_version}" \
		"${asset}.zip" SHASUMS256.txt "${tmp}"
	unzip -oq "${tmp}/${asset}.zip" -d "${tmp}"
	export BUN_INSTALL="${BUN_INSTALL:-$HOME/.bun}"
	mkdir -p "${BUN_INSTALL}/bin"
	install -m 755 "${tmp}/${asset}/bun" "${BUN_INSTALL}/bin/bun"
	rm -rf "${tmp}"
	export PATH="${BUN_INSTALL}/bin:${PATH}"
	if [ "$(bun --version)" != "${required_bun_version}" ]; then
		printf 'error: need Bun %s; found %s\n' "${required_bun_version}" "$(bun --version)" >&2
		exit 1
	fi
}

install_rust_packages() {
	log "Installing packages for the bundled FFmpeg build and Tauri host"
	run_as_root apt-get update -qq
	run_as_root env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends \
		build-essential ca-certificates clang curl nasm pkg-config libopus-dev \
		libgtk-3-dev libwebkit2gtk-4.1-dev libsoup-3.0-dev librsvg2-dev
}

# Media-lane fixtures and readback spawn ffmpeg/ffprobe; distro FFmpeg 6.x
# decodes edit lists and Opus pre-skip differently than the engine's FFmpeg 9.
ensure_readback_cli() {
	if have ffprobe && ffprobe -version | head -1 | grep -q 'version n\?9\.'; then
		log "Using FFmpeg 9 readback CLI"
		return
	fi
	local asset tmp
	case "$(uname -m)" in
		x86_64) asset=ffmpeg-n9.0-latest-linux64-gpl-9.0.tar.xz ;;
		aarch64 | arm64) asset=ffmpeg-n9.0-latest-linuxarm64-gpl-9.0.tar.xz ;;
		*) printf 'error: no FFmpeg CLI build for CPU %s\n' "$(uname -m)" >&2; exit 1 ;;
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

ensure_sidecar_stub() {
	local path
	path="${repo_root}/src-tauri/binaries/abb-aaxclean-helper-$(rustc -vV | awk '/^host:/ { print $2 }')"
	if [ ! -x "${path}" ]; then
		log "Creating AAXClean sidecar stub ${path}"
		mkdir -p "$(dirname "${path}")"
		printf '#!/usr/bin/env sh\nexit 0\n' > "${path}"
		chmod +x "${path}"
	fi
}

case "${mode}" in
	frontend)
		ensure_bun
		log "Installing frontend dependencies"
		(cd "${repo_root}" && bun install --frozen-lockfile)
		;;
	--rust)
		install_rust_packages
		ensure_readback_cli
		ensure_sidecar_stub
		;;
	*) printf 'error: unknown mode %s (expected --rust or no argument)\n' "${mode}" >&2; exit 1 ;;
esac
