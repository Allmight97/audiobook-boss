#!/bin/bash
# Homebrew owns installation, dependencies, updates, and removal. ABB only opens this handoff.

install_fdk() {
    local brew="$1" installed options
    local formula='homebrew-ffmpeg/ffmpeg/ffmpeg'
    installed=$("$brew" list --formula --full-name) || return
    if printf '%s\n' "$installed" | /usr/bin/grep -Eq '^(ffmpeg|homebrew/core/ffmpeg)$'; then
        printf '\nAnother Homebrew FFmpeg is already installed. Homebrew cannot install both formulas together.\n'
        printf 'Review your existing installation before changing it; ABB has made no changes.\n'
        printf 'Details: https://github.com/homebrew-ffmpeg/homebrew-ffmpeg\n'
        return 1
    fi
    if printf '%s\n' "$installed" | /usr/bin/grep -Fxq "$formula"; then
        options=$("$brew" info --json=v2 "$formula" | /usr/bin/plutil -extract formulae.0.installed.0.used_options json -o - -) || return
        if printf '%s\n' "$options" | /usr/bin/grep -Fq -- '--with-fdk-aac'; then
            printf '\nHomebrew will check for updates and upgrade its FDK-enabled FFmpeg if needed.\n'
            "$brew" update && "$brew" upgrade "$formula" || return
        else
            printf '\nHomebrew will rebuild its FFmpeg with FDK AAC, preserving existing build options.\n'
            "$brew" reinstall "$formula" --with-fdk-aac || return
        fi
    else
        printf '\nHomebrew will build FFmpeg with FDK AAC and install its dependencies. This can take a while.\n'
        "$brew" install "$formula" --with-fdk-aac || return
    fi
    printf '\nHomebrew finished. Return to AudioBook Boss Settings and click Recheck FDK to validate the encoder.\n'
}

main() {
    local brew
    printf 'AudioBook Boss — optional FDK AAC setup\n'
    printf 'Uses the community homebrew-ffmpeg formula. Homebrew manages the software on your Mac.\n'
    if ! /usr/bin/xcode-select -p >/dev/null 2>&1; then
        printf '\nInstall Apple Command Line Tools in the system dialog, then run this setup again.\n'
        /usr/bin/xcode-select --install
        return
    fi
    for brew in /opt/homebrew/bin/brew /usr/local/bin/brew; do
        if [ -x "$brew" ]; then
            install_fdk "$brew"
            return
        fi
    done
    printf '\nHomebrew is required. Opening its official installer download in your browser.\n'
    printf 'Open the downloaded Homebrew.pkg and finish installation, then run this setup again.\n'
    /usr/bin/open 'https://github.com/Homebrew/brew/releases/latest/download/Homebrew.pkg'
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then
    set -o pipefail
    main
fi
