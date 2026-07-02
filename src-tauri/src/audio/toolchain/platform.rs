//! Per-platform FFmpeg probe seam (vault.rs pattern): candidate enumeration
//! and binary-arch acceptance are the only platform-varying parts of toolchain
//! resolution. Dispatchers are `#[cfg(target_os)]`-gated; the platform *rules*
//! are unconditional pure functions so every platform's rules unit-test on any
//! host. Unsupported platforms fall back explicitly (empty candidates, binary
//! rejected) — never silently.
//!
//! Runtime proof on real Linux hardware (actual `file`/`pkg-config`/apt
//! ffmpeg) is deferred to the Linux port; this module ships reviewed rules.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
use std::ffi::OsStr;

const APPLE_SILICON_FFMPEG_ARCHES: &[&str] = &["arm64", "arm64e"];

// ---------------------------------------------------------------------------
// cfg-gated dispatchers — the only conditional code in this module.
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
pub(super) fn auto_candidates() -> Vec<PathBuf> {
    ordered_macos_auto_candidate_paths(
        super::first_successful_stdout(
            &["brew", "/opt/homebrew/bin/brew"],
            &["--prefix", "ffmpeg"],
        )
        .as_deref(),
        super::first_successful_stdout(
            &["pkg-config", "/opt/homebrew/bin/pkg-config"],
            &["--variable=prefix", "libavcodec"],
        )
        .as_deref(),
        super::first_successful_stdout(&["which", "/usr/bin/which"], &["ffmpeg"]).as_deref(),
    )
}

#[cfg(target_os = "linux")]
pub(super) fn auto_candidates() -> Vec<PathBuf> {
    ordered_linux_auto_candidate_paths(
        super::first_successful_stdout(
            &["pkg-config", "/usr/bin/pkg-config"],
            &["--variable=prefix", "libavcodec"],
        )
        .as_deref(),
        super::first_successful_stdout(&["which", "/usr/bin/which"], &["ffmpeg"]).as_deref(),
    )
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub(super) fn auto_candidates() -> Vec<PathBuf> {
    // Explicit empty: resolution reports "no external FFmpeg toolchain was
    // detected" rather than probing with rules that don't exist for this OS.
    Vec::new()
}

#[cfg(target_os = "macos")]
pub(super) fn is_supported_ffmpeg_binary(candidate: &Path) -> bool {
    if let Ok(arches) = super::probe_stdout("lipo", [OsStr::new("-archs"), candidate.as_os_str()]) {
        return is_supported_macos_arch_listing(&arches);
    }

    if let Ok(description) = super::probe_stdout("file", [OsStr::new("-b"), candidate.as_os_str()])
    {
        return is_supported_macos_file_description(&description);
    }

    false
}

#[cfg(target_os = "linux")]
pub(super) fn is_supported_ffmpeg_binary(candidate: &Path) -> bool {
    use std::ffi::OsStr;
    if let Ok(description) = super::probe_stdout("file", [OsStr::new("-b"), candidate.as_os_str()])
    {
        return is_supported_linux_file_description(&description);
    }

    false
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub(super) fn is_supported_ffmpeg_binary(_candidate: &Path) -> bool {
    false
}

/// Rejection clause appended to "FFmpeg executable '<path>' …" when the
/// binary check fails. Kept per-platform so the error is truthful about what
/// was actually required.
#[cfg(target_os = "macos")]
pub(super) fn unsupported_binary_rejection_clause() -> &'static str {
    "is not an Apple Silicon binary (expected arm64 or arm64e)"
}

#[cfg(target_os = "linux")]
pub(super) fn unsupported_binary_rejection_clause() -> &'static str {
    "is not an x86-64 Linux binary (expected ELF 64-bit x86-64)"
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub(super) fn unsupported_binary_rejection_clause() -> &'static str {
    "cannot be validated: external FFmpeg toolchains are unsupported on this platform"
}

// ---------------------------------------------------------------------------
// Pure per-platform rules — compiled unconditionally, unit-tested on any host.
//
// Filesystem canonicalization is INJECTED so the ordering/dedupe rules stay
// genuinely pure: production passes `fs_canonicalize` (collapses symlinked
// candidates like Homebrew's opt/ links); tests pass a no-op so the proof is
// host-independent instead of depending on which paths exist locally.
// ---------------------------------------------------------------------------

type Canonicalize<'a> = &'a dyn Fn(&Path) -> Option<PathBuf>;

fn fs_canonicalize(path: &Path) -> Option<PathBuf> {
    path.canonicalize().ok()
}

fn push_candidate(
    candidates: &mut Vec<PathBuf>,
    seen: &mut HashSet<PathBuf>,
    candidate: PathBuf,
    canonicalize: Canonicalize,
) {
    if let Some(canonical) = canonicalize(&candidate) {
        if seen.insert(canonical.clone()) {
            candidates.push(canonical);
        }
        return;
    }

    if seen.insert(candidate.clone()) {
        candidates.push(candidate);
    }
}

pub(super) fn ordered_macos_auto_candidate_paths(
    brew_prefix: Option<&str>,
    pkg_config_prefix: Option<&str>,
    path_ffmpeg: Option<&str>,
) -> Vec<PathBuf> {
    ordered_macos_auto_candidate_paths_with(
        brew_prefix,
        pkg_config_prefix,
        path_ffmpeg,
        &fs_canonicalize,
    )
}

fn ordered_macos_auto_candidate_paths_with(
    brew_prefix: Option<&str>,
    pkg_config_prefix: Option<&str>,
    path_ffmpeg: Option<&str>,
    canonicalize: Canonicalize,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    if let Some(prefix) = brew_prefix.filter(|prefix| is_supported_macos_auto_detect_prefix(prefix))
    {
        push_candidate(
            &mut candidates,
            &mut seen,
            Path::new(prefix).join("bin/ffmpeg"),
            canonicalize,
        );
    }

    if let Some(prefix) =
        pkg_config_prefix.filter(|prefix| is_supported_macos_auto_detect_prefix(prefix))
    {
        push_candidate(
            &mut candidates,
            &mut seen,
            Path::new(prefix).join("bin/ffmpeg"),
            canonicalize,
        );
    }

    if let Some(path) = path_ffmpeg {
        push_candidate(
            &mut candidates,
            &mut seen,
            PathBuf::from(path),
            canonicalize,
        );
    }

    for path in [
        "/opt/homebrew/opt/ffmpeg/bin/ffmpeg",
        "/opt/homebrew/bin/ffmpeg",
    ] {
        push_candidate(
            &mut candidates,
            &mut seen,
            PathBuf::from(path),
            canonicalize,
        );
    }

    candidates
}

// The Linux rules compile on every host so they stay unit-tested from macOS;
// only the Linux dispatcher calls them at runtime.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(super) fn ordered_linux_auto_candidate_paths(
    pkg_config_prefix: Option<&str>,
    which_ffmpeg: Option<&str>,
) -> Vec<PathBuf> {
    ordered_linux_auto_candidate_paths_with(pkg_config_prefix, which_ffmpeg, &fs_canonicalize)
}

fn ordered_linux_auto_candidate_paths_with(
    pkg_config_prefix: Option<&str>,
    which_ffmpeg: Option<&str>,
    canonicalize: Canonicalize,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    if let Some(prefix) =
        pkg_config_prefix.filter(|prefix| is_supported_linux_auto_detect_prefix(prefix))
    {
        push_candidate(
            &mut candidates,
            &mut seen,
            Path::new(prefix).join("bin/ffmpeg"),
            canonicalize,
        );
    }

    if let Some(path) = which_ffmpeg {
        push_candidate(
            &mut candidates,
            &mut seen,
            PathBuf::from(path),
            canonicalize,
        );
    }

    for path in ["/usr/bin/ffmpeg", "/usr/local/bin/ffmpeg"] {
        push_candidate(
            &mut candidates,
            &mut seen,
            PathBuf::from(path),
            canonicalize,
        );
    }

    candidates
}

pub(super) fn is_supported_macos_auto_detect_prefix(prefix: &str) -> bool {
    prefix == "/opt/homebrew" || prefix.starts_with("/opt/homebrew/")
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(super) fn is_supported_linux_auto_detect_prefix(prefix: &str) -> bool {
    prefix == "/usr" || prefix == "/usr/local" || prefix.starts_with("/usr/")
}

pub(super) fn matches_supported_apple_silicon_arch(candidate_arch: &str) -> bool {
    candidate_arch == "arm64" || candidate_arch.starts_with("arm64e")
}

pub(super) fn is_supported_macos_arch_listing(arches: &str) -> bool {
    arches
        .split_whitespace()
        .any(matches_supported_apple_silicon_arch)
}

fn is_script_file_description(description: &str) -> bool {
    description.contains("script") || description.contains("text executable")
}

pub(super) fn is_supported_macos_file_description(description: &str) -> bool {
    if is_script_file_description(description) {
        return true;
    }
    APPLE_SILICON_FFMPEG_ARCHES
        .iter()
        .any(|expected| description.contains(expected))
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(super) fn is_supported_linux_file_description(description: &str) -> bool {
    if is_script_file_description(description) {
        return true;
    }
    description.contains("ELF 64-bit")
        && (description.contains("x86-64") || description.contains("x86_64"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests inject a no-op canonicalizer so ordering/dedupe proof does not
    /// depend on which paths exist (or are symlinked) on the test host.
    fn no_canonicalize(_: &Path) -> Option<PathBuf> {
        None
    }

    #[test]
    fn linux_candidates_order_pkg_config_then_which_then_heuristics() {
        let candidates = ordered_linux_auto_candidate_paths_with(
            Some("/usr"),
            Some("/tmp/abb-which-linux-ffmpeg"),
            &no_canonicalize,
        );

        // pkg-config prefix first, then which, then the one heuristic the
        // literal dedupe leaves (/usr/bin/ffmpeg already emitted).
        assert_eq!(
            candidates,
            vec![
                PathBuf::from("/usr/bin/ffmpeg"),
                PathBuf::from("/tmp/abb-which-linux-ffmpeg"),
                PathBuf::from("/usr/local/bin/ffmpeg"),
            ]
        );
    }

    #[test]
    fn linux_candidates_reject_unsupported_prefixes() {
        let candidates =
            ordered_linux_auto_candidate_paths_with(Some("/opt/homebrew"), None, &no_canonicalize);

        assert!(
            candidates
                .iter()
                .all(|candidate| !candidate.to_string_lossy().contains("/opt/homebrew")),
            "Homebrew prefixes are not Linux auto-detect sources: {candidates:?}"
        );
        assert_eq!(
            candidates,
            vec![
                PathBuf::from("/usr/bin/ffmpeg"),
                PathBuf::from("/usr/local/bin/ffmpeg"),
            ]
        );
    }

    #[test]
    fn linux_prefix_rule_accepts_usr_roots_and_rejects_foreign_roots() {
        assert!(is_supported_linux_auto_detect_prefix("/usr"));
        assert!(is_supported_linux_auto_detect_prefix("/usr/local"));
        assert!(is_supported_linux_auto_detect_prefix(
            "/usr/lib/x86_64-linux-gnu"
        ));
        assert!(!is_supported_linux_auto_detect_prefix("/opt/homebrew"));
        assert!(!is_supported_linux_auto_detect_prefix(
            "/snap/ffmpeg/current"
        ));
        assert!(!is_supported_linux_auto_detect_prefix("relative/junk"));
        assert!(!is_supported_linux_auto_detect_prefix("/usrx"));
    }

    #[test]
    fn linux_file_description_rule_accepts_x86_64_elf_only() {
        assert!(is_supported_linux_file_description(
            "ELF 64-bit LSB pie executable, x86-64, version 1 (SYSV), dynamically linked"
        ));
        assert!(is_supported_linux_file_description(
            "ELF 64-bit LSB executable, x86_64, statically linked"
        ));
        assert!(!is_supported_linux_file_description(
            "ELF 32-bit LSB executable, Intel 80386"
        ));
        assert!(!is_supported_linux_file_description(
            "ELF 64-bit LSB executable, ARM aarch64"
        ));
        assert!(!is_supported_linux_file_description(
            "Mach-O 64-bit executable arm64"
        ));
        // Script passthrough parity with macOS: fake/wrapper ffmpeg scripts
        // stay validatable by the probe chain, not the arch gate.
        assert!(is_supported_linux_file_description(
            "a /bin/sh script, ASCII text executable"
        ));
    }

    #[test]
    fn macos_rules_keep_current_behavior_after_extraction() {
        // Pins that the seam extraction changed nothing on macOS. Ordering is
        // asserted with canonicalization stubbed out: brew prefix, then
        // pkg-config prefix, then which; the hardcoded tail dedupes away.
        let candidates = ordered_macos_auto_candidate_paths_with(
            Some("/opt/homebrew/opt/ffmpeg"),
            Some("/opt/homebrew"),
            Some("/tmp/abb-which-ffmpeg"),
            &no_canonicalize,
        );
        assert_eq!(
            candidates,
            vec![
                PathBuf::from("/opt/homebrew/opt/ffmpeg/bin/ffmpeg"),
                PathBuf::from("/opt/homebrew/bin/ffmpeg"),
                PathBuf::from("/tmp/abb-which-ffmpeg"),
            ]
        );

        assert!(is_supported_macos_auto_detect_prefix("/opt/homebrew"));
        assert!(!is_supported_macos_auto_detect_prefix("/usr/local"));

        assert!(matches_supported_apple_silicon_arch("arm64"));
        assert!(matches_supported_apple_silicon_arch("arm64e"));
        assert!(!matches_supported_apple_silicon_arch("aarch64"));
        assert!(!matches_supported_apple_silicon_arch("x86_64"));

        assert!(is_supported_macos_arch_listing("x86_64 arm64"));
        assert!(!is_supported_macos_arch_listing("x86_64"));

        assert!(is_supported_macos_file_description(
            "Mach-O 64-bit executable arm64"
        ));
        assert!(!is_supported_macos_file_description(
            "ELF 64-bit LSB executable, x86-64"
        ));
        assert!(is_supported_macos_file_description(
            "a /bin/sh script, ASCII text executable"
        ));
    }

    #[test]
    fn rejection_clause_names_the_running_platform_requirement() {
        let clause = unsupported_binary_rejection_clause();
        #[cfg(target_os = "macos")]
        assert_eq!(
            clause,
            "is not an Apple Silicon binary (expected arm64 or arm64e)"
        );
        #[cfg(target_os = "linux")]
        assert_eq!(
            clause,
            "is not an x86-64 Linux binary (expected ELF 64-bit x86-64)"
        );
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        assert!(clause.contains("unsupported on this platform"));
    }
}
