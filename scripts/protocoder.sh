#!/usr/bin/env bash

# Purpose: Personal prototype for testing audiobook AAC encoding profiles
# Encoders: libfdk_aac (HE-AAC) and FFmpeg native AAC
# Notes: Designed for macOS personal use; filenames may include spaces

set -euo pipefail

# =============================================================================
# CONFIGURATION
# =============================================================================

# Quality profiles: [bitrate, cutoff_freq]
# Research note: HE-AAC at ~56k often matches AAC-LC ~80k for speech
declare -A FDK_PROFILES=(
    ["low"]="48k:18000"      # Maximum compression
    ["medium"]="56k:18000"    # Balanced (default)
    ["high"]="64k:18000"      # Higher quality
)

# Native AAC VBR quality settings (q:a values)
# Approximate mono results: low ~48-56k, med ~64-72k, high ~80-96k
declare -A NATIVE_VBR_PROFILES=(
    ["low"]="0.5:15000"
    ["medium"]="0.7:16000"
    ["high"]="1.0:17000"
)

# Native AAC CBR settings (for comparison)
declare -A NATIVE_CBR_PROFILES=(
    ["low"]="56k:15000"
    ["medium"]="72k:16000"
    ["high"]="96k:17000"
)

# Defaults and env-driven toggles
ENCODER="${1:-fdk}"            # fdk | native | test
QUALITY="${2:-medium}"         # low | medium | high
INPUT_FILE="${3:-}"            # optional; batch if empty
AFTERBURNER="${AFTERBURNER:-1}" # FDK afterburner (0/1)
PREVIEW_MODE="${PREVIEW:-0}"     # 1: show command only
SAMPLE_MODE="${SAMPLE:-0}"       # 1: encode first 30s only
NATIVE_VBR="${VBR:-1}"           # 1: VBR (default) | 0: CBR
MONO_OUTPUT=1                      # Force mono for audiobooks

# Logging
LOG_DIR="./logs"
LOG_FILE="${LOG_DIR}/encode_$(date +%Y%m%d_%H%M%S).log"

# Working command array (set by build_encode_command)
CMD=()

# =============================================================================
# UTILITIES
# =============================================================================

check_requirements() {
    command -v ffmpeg >/dev/null || { echo "Error: ffmpeg not installed" >&2; exit 1; }
    command -v ffprobe >/dev/null || { echo "Error: ffprobe not installed" >&2; exit 1; }

    # If FDK requested, verify availability; fall back to native if missing
    if [[ "$ENCODER" == "fdk" ]]; then
        if ! ffmpeg -hide_banner -encoders 2>/dev/null | grep -q "libfdk_aac"; then
            echo "Note: libfdk_aac not available (ensure your ffmpeg includes it)." >&2
            echo "Personal dev note: on macOS/Homebrew you may need a custom build/tap." >&2
            echo "Falling back to native AAC..." >&2
            ENCODER="native"
        fi
    fi
}

# Probe helpers
probe_stream_property() {
    local file="$1" stream_sel="$2" entry="$3"
    ffprobe -v error -select_streams "$stream_sel" -show_entries "$entry" -of default=noprint_wrappers=1:nokey=1 -- "$file" 2>/dev/null | head -n1
}

probe_format_property() {
    local file="$1" entry="$2"
    ffprobe -v error -show_entries "$entry" -of default=noprint_wrappers=1:nokey=1 -- "$file" 2>/dev/null | head -n1
}

get_audio_channels() { probe_stream_property "$1" "a:0" "stream=channels"; }
get_audio_codec()    { probe_stream_property "$1" "a:0" "stream=codec_name"; }
get_format_name()    { probe_format_property  "$1" "format=format_name"; }

calculate_bitrate_kbps() {
    local file="$1"
    local size_bytes duration_sec

    size_bytes=$(stat -f%z -- "$file" 2>/dev/null || stat -c%s -- "$file" 2>/dev/null || echo 0)
    duration_sec=$(probe_format_property "$file" "format=duration")

    awk -v bytes="$size_bytes" -v dur="$duration_sec" '
        BEGIN { if (dur > 0) print int((bytes * 8) / (dur * 1000)); else print 0 }
    '
}

get_encoder_settings() {
    local encoder="$1" quality="$2"
    if [[ "$encoder" == "fdk" ]]; then
        echo "${FDK_PROFILES[$quality]:-${FDK_PROFILES[medium]}}"
    elif [[ "$NATIVE_VBR" == "1" ]]; then
        echo "${NATIVE_VBR_PROFILES[$quality]:-${NATIVE_VBR_PROFILES[medium]}}"
    else
        echo "${NATIVE_CBR_PROFILES[$quality]:-${NATIVE_CBR_PROFILES[medium]}}"
    fi
}

# Compute whether input already meets target container/codec/channels/bitrate
is_compliant_format() {
    local file="$1"
    local format codec channels bitrate target_bitrate settings

    format=$(get_format_name "$file")
    case "$format" in
        *mp4*|*m4a*|*m4b*) :;;
        *) return 1;;
    esac

    codec=$(get_audio_codec "$file")
    [[ "$codec" == "aac" ]] || return 1

    channels=$(get_audio_channels "$file")
    [[ "$channels" == "1" ]] || return 1

    bitrate=$(calculate_bitrate_kbps "$file")
    settings=$(get_encoder_settings "$ENCODER" "$QUALITY")

    # Determine numeric kbps target for comparison
    if [[ "$ENCODER" == "fdk" ]]; then
        target_bitrate="${settings%%:*}"; target_bitrate="${target_bitrate%k}"
    elif [[ "$NATIVE_VBR" == "1" ]]; then
        case "$QUALITY" in
            low) target_bitrate=56;;
            medium) target_bitrate=72;;
            high) target_bitrate=96;;
        esac
    else
        target_bitrate="${settings%%:*}"; target_bitrate="${target_bitrate%k}"
    fi

    # Skip when bitrate already at/under target (+10% tolerance)
    (( bitrate <= target_bitrate + target_bitrate/10 ))
}

# Build the ffmpeg command in global CMD[]; print a preview string if needed
build_encode_command() {
    local input="$1" output="$2" settings="$3"
    IFS=':' read -r bitrate_or_quality cutoff <<<"$settings"

    CMD=(ffmpeg -hide_banner -i "$input")

    # Sample mode: encode only first 30 seconds
    [[ "$SAMPLE_MODE" == "1" ]] && CMD+=(-t 30)

    if [[ "$ENCODER" == "fdk" ]]; then
        CMD+=(
            -c:a libfdk_aac
            -profile:a aac_he
            -afterburner "$AFTERBURNER"
            -b:a "$bitrate_or_quality"
            -cutoff "$cutoff"
        )
    else
        CMD+=( -c:a aac -aac_coder twoloop )
        if [[ "$NATIVE_VBR" == "1" ]]; then
            CMD+=( -q:a "$bitrate_or_quality" )
        else
            CMD+=( -b:a "$bitrate_or_quality" )
        fi
        CMD+=( -cutoff "$cutoff" )
    fi

    [[ "$MONO_OUTPUT" == "1" ]] && CMD+=( -ac 1 )

    CMD+=( -map_metadata 0 -map_chapters 0 -movflags +faststart -y "$output" )
}

preview_cmd() {
    # Print a shell-escaped command preview
    printf '%q ' "${CMD[@]}"
    echo
}

encode_file() {
    local input="$1"
    local base_name="${input%.*}"

    local output="${base_name}_${ENCODER}"
    [[ "$ENCODER" == "native" && "$NATIVE_VBR" == "1" ]] && output+="_vbr"
    [[ "$ENCODER" == "native" && "$NATIVE_VBR" == "0" ]] && output+="_cbr"
    [[ "$SAMPLE_MODE" == "1" ]] && output+="_sample"
    output+="_${QUALITY}.m4b"

    # Existing output: skip
    if [[ -f "$output" ]]; then
        echo "Skipping (output exists): $(basename -- "$output")"
        return 0
    fi

    # Already compliant: skip
    if is_compliant_format "$input"; then
        echo "Skipping (already compliant): $input"
        return 0
    fi

    local settings
    settings=$(get_encoder_settings "$ENCODER" "$QUALITY")

    build_encode_command "$input" "$output" "$settings"

    if [[ "$PREVIEW_MODE" == "1" ]]; then
        echo "Preview mode - Command to execute:"
        preview_cmd
        return 0
    fi

    mkdir -p "$LOG_DIR"
    echo "=== Encoding: $input ===" | tee -a "$LOG_FILE"
    local encoder_mode="$ENCODER"
    [[ "$ENCODER" == "native" ]] && encoder_mode+=" ($([[ "$NATIVE_VBR" == "1" ]] && echo VBR || echo CBR))"
    echo "Encoder: $encoder_mode ($QUALITY profile)" | tee -a "$LOG_FILE"
    [[ "$SAMPLE_MODE" == "1" ]] && echo "Sample mode: 30 seconds only" | tee -a "$LOG_FILE"

    if [[ "$ENCODER" == "native" && "$NATIVE_VBR" == "1" ]]; then
        echo "Settings: VBR q=${settings%%:*} @ ${settings##*:}Hz" | tee -a "$LOG_FILE"
    else
        echo "Settings: CBR ${settings//:/ @ }Hz" | tee -a "$LOG_FILE"
    fi

    local start_time end_time duration
    start_time=$(date +%s)

    if "${CMD[@]}" 2>&1 | tee -a "$LOG_FILE"; then
        end_time=$(date +%s)
        duration=$((end_time - start_time))
        if   [[ $duration -lt 1  ]]; then echo "✓ Done (instant)" | tee -a "$LOG_FILE"
        elif [[ $duration -lt 60 ]]; then echo "✓ Done in ${duration}s" | tee -a "$LOG_FILE"
        else echo "✓ Done in $((duration/60))m $((duration%60))s" | tee -a "$LOG_FILE"; fi

        if [[ -f "$input" && -f "$output" ]]; then
            local input_size output_size reduction
            input_size=$(stat -f%z -- "$input" 2>/dev/null || stat -c%s -- "$input")
            output_size=$(stat -f%z -- "$output" 2>/dev/null || stat -c%s -- "$output")
            if [[ "$input_size" -gt 0 ]]; then
                reduction=$(( 100 - (output_size * 100 / input_size) ))
                echo "Size reduction: ${reduction}%" | tee -a "$LOG_FILE"
            fi
        fi
    else
        echo "✗ Encoding failed" | tee -a "$LOG_FILE"
        return 1
    fi
}

run_quality_tests() {
    local input="$1"
    echo "Running quality comparison tests..."
    echo "Input: $input"
    [[ "$SAMPLE_MODE" == "1" ]] && echo "Sample mode: 30-second samples only"
    echo

    for encoder in fdk native; do
        ENCODER="$encoder"

        if [[ "$encoder" == "fdk" ]] && ! ffmpeg -hide_banner -encoders 2>/dev/null | grep -q "libfdk_aac"; then
            echo "Skipping FDK tests (not available)"
            continue
        fi

        if [[ "$encoder" == "native" ]]; then
            for vbr_mode in 1 0; do
                NATIVE_VBR="$vbr_mode"
                local mode_name
                mode_name=$([[ "$vbr_mode" == "1" ]] && echo "VBR" || echo "CBR")
                for quality in low medium high; do
                    QUALITY="$quality"
                    echo "--- Testing: native $mode_name @ $quality ---"
                    encode_file "$input"
                    echo
                done
            done
        else
            for quality in low medium high; do
                QUALITY="$quality"
                echo "--- Testing: $encoder @ $quality ---"
                encode_file "$input"
                echo
            done
        fi
    done
}

show_usage() {
    cat <<EOF
Usage: $0 [encoder] [quality] [input_file]
       $0 [encoder] [quality]
       $0 test [input_file]

Encoders:
  fdk     - libfdk_aac with HE-AAC profile (CBR)
  native  - FFmpeg native AAC with twoloop (VBR default)

Quality:
  low | medium | high

Environment variables:
  AFTERBURNER=0|1  - FDK afterburner (default: 1)
  VBR=0|1          - Native AAC mode (default: 1=VBR, 0=CBR)
  PREVIEW=1        - Show command without executing
  SAMPLE=1         - Encode first 30 seconds only

Examples:
  $0 native medium                      # Batch: encode all supported files
  $0 fdk low audiobook.mp3              # Single: encode specific file
  SAMPLE=1 $0 native medium             # Batch: 30-sec samples for testing
  VBR=0 $0 native medium book.mp3       # Single: Native AAC CBR 72k
  SAMPLE=1 $0 test audiobook.mp3        # Test all settings on a file
  PREVIEW=1 $0 fdk low test.mp3         # Show command only
EOF
}

main() {
    if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
        show_usage; exit 0
    fi

    if [[ "${1:-}" == "test" ]]; then
        if [[ -z "${2:-}" ]]; then
            echo "Error: test mode requires an input file" >&2
            show_usage; exit 1
        fi
        check_requirements
        local sample_setting="$SAMPLE_MODE"; SAMPLE_MODE="$sample_setting"
        run_quality_tests "$2"
        exit 0
    fi

    if [[ "$ENCODER" != "fdk" && "$ENCODER" != "native" ]]; then
        echo "Error: encoder must be 'fdk' or 'native'" >&2
        show_usage; exit 1
    fi

    check_requirements

    # Batch mode when no specific input file is given
    if [[ -z "$INPUT_FILE" ]]; then
        # Avoid literal globs when no matches
        local restore_nullglob
        restore_nullglob=$(shopt -p nullglob || true)
        shopt -s nullglob
        local -a audio_files=( *.m4b *.mp3 *.m4a )
        # Restore previous state
        eval "$restore_nullglob" || true

        if [[ ${#audio_files[@]} -eq 0 ]]; then
            echo "No audio files found (*.m4b *.mp3 *.m4a)" >&2
            exit 1
        fi

        echo "Batch mode: Processing ${#audio_files[@]} file(s)"
        echo "Encoder: $ENCODER | Quality: $QUALITY"
        [[ "$ENCODER" == "native" ]] && echo "Mode: $([[ "$NATIVE_VBR" == "1" ]] && echo "VBR" || echo "CBR")"
        [[ "$SAMPLE_MODE" == "1" ]] && echo "Sample mode: 30-second samples only"
        echo

        for file in "${audio_files[@]}"; do
            [[ -f "$file" ]] || continue
            echo "[$(date +%H:%M:%S)] Processing: $file"
            encode_file "$file"
            echo
        done

        echo "Batch processing complete!"
        exit 0
    fi

    # Single file mode
    if [[ ! -f "$INPUT_FILE" ]]; then
        echo "Error: file not found: $INPUT_FILE" >&2
        exit 1
    fi

    encode_file "$INPUT_FILE"
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi

