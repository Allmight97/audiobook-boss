#!/usr/bin/env bash
set -euo pipefail

# Codex Cloud and Codex-managed worktree setup for ABB.
# Intended cloud setup command:
#   bash scripts/setup-codex-agent-env.sh
#
# The script is idempotent and installs/builds only environment prerequisites.
# It does not run the proof suite; agents should choose proof commands from
# scripts/AGENTS.md by touched owner.

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ffmpeg_ref="${ABB_CODEX_FFMPEG_REF:-release/8.1}"
ffmpeg_prefix="${ABB_CODEX_FFMPEG_PREFIX:-/opt/ffmpeg81}"
ffmpeg_src="${ABB_CODEX_FFMPEG_SRC:-/opt/ffmpeg-src}"
local_env_file="${repo_root}/.codex/agent-env.local.sh"
required_bun_version="1.3.14"

log() {
	printf '\n==> %s\n' "$*"
}

warn() {
	printf 'warning: %s\n' "$*" >&2
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

cpu_count() {
	if have nproc; then
		nproc
	elif have sysctl; then
		sysctl -n hw.ncpu
	else
		printf '2\n'
	fi
}

host_triple() {
	if have rustc; then
		local rustc_version
		rustc_version="$(rustc -vV)"
		printf '%s\n' "${rustc_version}" | awk '/^host:/ { print $2; exit }'
		return
	fi

	case "$(uname -s):$(uname -m)" in
		Linux:x86_64) printf 'x86_64-unknown-linux-gnu\n' ;;
		Darwin:arm64) printf 'aarch64-apple-darwin\n' ;;
		Darwin:x86_64) printf 'x86_64-apple-darwin\n' ;;
		*) printf 'unsupported\n' ;;
	esac
}

ensure_rust_toolchain() {
	if ! have rustup; then
		warn "rustup not found; relying on the image's Rust toolchain"
		return
	fi

	log "Installing Rust toolchain from rust-toolchain.toml"
	rustup toolchain install 1.95 --component rustfmt --component clippy
}

ensure_bun() {
	if have bun; then
		log "Using existing Bun $(bun --version)"
		if [ "$(bun --version)" != "${required_bun_version}" ]; then
			warn "repo packageManager pins Bun ${required_bun_version}; prefer setting that version in Codex environment settings"
		fi
		return
	fi

	if ! have curl; then
		warn "curl not found; cannot install Bun automatically"
		return
	fi

	log "Installing Bun for frontend dependency setup"
	curl -fsSL https://bun.sh/install | bash
	export BUN_INSTALL="${BUN_INSTALL:-$HOME/.bun}"
	export PATH="${BUN_INSTALL}/bin:${PATH}"
	if have bun && [ "$(bun --version)" != "${required_bun_version}" ]; then
		warn "installed Bun $(bun --version); repo packageManager pins ${required_bun_version}"
	fi
}

ensure_sidecar_stub() {
	local triple
	local path

	triple="$(host_triple)"
	if [ "${triple}" = "unsupported" ] || [ -z "${triple}" ]; then
		warn "unable to infer host triple for AAXClean sidecar stub"
		return
	fi

	path="${repo_root}/src-tauri/binaries/abb-aaxclean-helper-${triple}"
	if [ -x "${path}" ]; then
		log "AAXClean sidecar stub already exists for ${triple}"
		return
	fi

	log "Creating AAXClean sidecar stub for ${triple}"
	mkdir -p "${repo_root}/src-tauri/binaries"
	printf '#!/usr/bin/env sh\nexit 0\n' > "${path}"
	chmod +x "${path}"
}

persist_linux_ffmpeg_paths() {
	local source_line="[ -f \"${local_env_file}\" ] && . \"${local_env_file}\""

	log "Writing FFmpeg environment file"
	mkdir -p "$(dirname "${local_env_file}")"
	{
		printf 'export PKG_CONFIG_PATH="%s${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"\n' "${ffmpeg_prefix}/lib/pkgconfig"
		printf 'export LD_LIBRARY_PATH="%s${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"\n' "${ffmpeg_prefix}/lib"
	} > "${local_env_file}"

	touch "${HOME}/.bashrc"
	if ! grep -Fq "${local_env_file}" "${HOME}/.bashrc"; then
		{
			printf '\n# ABB Codex agent FFmpeg discovery\n'
			printf '%s\n' "${source_line}"
		} >> "${HOME}/.bashrc"
	fi

	if [ "$(id -u)" -eq 0 ] || have sudo; then
		log "Registering FFmpeg ${ffmpeg_prefix} for runtime linker discovery"
		run_as_root mkdir -p /etc/ld.so.conf.d /etc/profile.d
		printf '%s/lib\n' "${ffmpeg_prefix}" | run_as_root tee /etc/ld.so.conf.d/abb-ffmpeg81.conf >/dev/null
		cat "${local_env_file}" | run_as_root tee /etc/profile.d/abb-codex-agent-env.sh >/dev/null
		run_as_root ldconfig
	else
		warn "source ${local_env_file} before cargo commands in this non-root setup"
	fi
}

install_linux_packages() {
	log "Installing Linux packages for Tauri, FFmpeg, bindgen, and media fixtures"
	run_as_root apt-get update
	run_as_root apt-get install -y --no-install-recommends \
		build-essential ca-certificates curl git make pkg-config \
		clang nasm libmp3lame-dev \
		libgtk-3-dev libwebkit2gtk-4.1-dev libsoup-3.0-dev librsvg2-dev
}

ensure_linux_ffmpeg() {
	if [ -f "${ffmpeg_prefix}/lib/pkgconfig/libavcodec.pc" ]; then
		log "Using existing FFmpeg at ${ffmpeg_prefix}"
		persist_linux_ffmpeg_paths
		return
	fi

	log "Building FFmpeg ${ffmpeg_ref} into ${ffmpeg_prefix}"
	run_as_root rm -rf "${ffmpeg_src}"
	run_as_root git clone --depth=1 -b "${ffmpeg_ref}" https://github.com/FFmpeg/FFmpeg "${ffmpeg_src}"

	pushd "${ffmpeg_src}" >/dev/null
	run_as_root ./configure \
		--prefix="${ffmpeg_prefix}" \
		--enable-shared \
		--disable-static \
		--disable-doc \
		--enable-libmp3lame
	run_as_root make -j"$(cpu_count)"
	run_as_root make install
	popd >/dev/null

	persist_linux_ffmpeg_paths
}

setup_linux() {
	install_linux_packages
	ensure_rust_toolchain
	ensure_linux_ffmpeg
	ensure_bun
	ensure_sidecar_stub
}

setup_macos() {
	if ! have brew; then
		warn "Homebrew not found; install FFmpeg and pkg-config before running Rust media proof"
	else
		log "Installing macOS packages for FFmpeg-backed proof"
		brew list ffmpeg >/dev/null 2>&1 || brew install ffmpeg
		brew list pkg-config >/dev/null 2>&1 || brew install pkg-config
	fi

	ensure_rust_toolchain
	ensure_bun
	ensure_sidecar_stub
}

install_frontend_deps() {
	if ! have bun; then
		warn "Bun unavailable; skipping bun install"
		return
	fi

	log "Installing frontend dependencies"
	(
		cd "${repo_root}"
		bun install --frozen-lockfile
	)
}

case "$(uname -s)" in
	Linux) setup_linux ;;
	Darwin) setup_macos ;;
	*) printf 'error: unsupported OS for ABB Codex agent setup: %s\n' "$(uname -s)" >&2; exit 1 ;;
esac

install_frontend_deps

log "Codex agent environment setup complete"
