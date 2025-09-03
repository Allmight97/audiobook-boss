#!/usr/bin/env bash

# Purpose: Test AAC encoding options for audiobook compression
# Focus: libfdk_aac (HE-AAC) vs native AAC for optimal quality/size ratio
# Environment: macOS personal testing

set -euo pipefail

# =============================================================================
# CONFIGURATION - Easy to modify for testing different settings
# =============================================================================

# Quality profiles: [bitrate, cutoff_freq]
# Research: HE-AAC at 56k ≈ AAC-LC at 80k for speech
declare -A FDK_PROFILES=(
    ["low"]="48k:18000"      # Maximum compression
    ["medium"]="56k:18000"    # Balanced (default)
    ["high"]="64k:18000"      # Premium quality
)

# Native AAC VBR quality settings (q:a values)
# VBR produces ~48-56k (low), ~64-72k (med), ~80-96k (high) for mono
declare -A NATIVE_VBR_PROFILES=(
    ["low"]="0.5:15000"      # ~48-56k mono
    ["medium"]="0.7:16000"    # ~64-72k mono
    ["high"]="1.0:17000"      # ~80-96k mono
)

# Native AAC CBR fallback (for comparison)
declare -A NATIVE_CBR_PROFILES=(
    ["low"]="56k:15000"       
    ["medium"]="72k:16000"    
    ["high"]="96k:17000"
)

# Default settings
ENCODER="${1:-fdk}"           # fdk or native
QUALITY="${2:-medium}"        # low, medium, high
INPUT_FILE="${3:-}"          # Input audio file (optional - batch if empty)
AFTERBURNER="${AFTERBURNER:-1}"  # FDK afterburner (0/1)
PREVIEW_MODE="${PREVIEW:-0}"     # Show command without executing
SAMPLE_MODE="${SAMPLE:-0}"       # Encode only first 30 seconds
NATIVE_VBR="${VBR:-1}"           # Use VBR for native AAC (0=CBR, 1=VBR)
MONO_OUTPUT=1                     # Force mono for audiobooks

# Logging
LOG_DIR="./logs"
LOG_FILE="${LOG_DIR}/encode_$(date +%Y%m%d_%H%M%S).log"

# =============================================================================
# CORE FUNCTIONS - Focused on essential encoding logic
# =============================================================================

check_requirements() {
    if ! command -v ffmpeg &> /dev/null; then
        echo "Error: ffmpeg not installed" >&2
        exit 1
    fi
    
    if ! command -v ffprobe &> /dev/null; then
        echo "Error: ffprobe not installed" >&2
        exit 1
    fi
    
    # Check for requested encoder
    if [[ "$ENCODER" == "fdk" ]]; then
        if ! ffmpeg -hide_banner -encoders 2>/dev/null | grep -q "libfdk_aac"; then
            echo "Error: libfdk_aac not available. Install with: brew install ffmpeg --with-fdk-aac" >&2
            echo "Falling back to native AAC..." >&2
            ENCODER="native"
        fi
    fi
}

# Probe file properties for skip logic
probe_property() {
    local file="$1" stream_type="$2" property="$3"
    ffprobe -v error -select_streams "$stream_type" -show_entries "$property" -of default=nw=1:nk=1 -- "$file" 2>/dev/null | head -n1
}

get_audio_channels() {
    probe_property "$1" "a:0" "stream=channels"
}

get_audio_codec() {
    probe_property "$1" "a:0" "stream=codec_name"
}

get_format_name() {
    probe_property "$1" "" "format=format_name"
}

calculate_bitrate_kbps() {
    local file="$1"
    local size_bytes duration_sec
    
    size_bytes=$(stat -f%z -- "$file" 2>/dev/null || stat -c%s -- "$file" 2>/dev/null || echo 0)
    duration_sec=$(probe_property "$file" "a:0" "format=duration")
    
    awk -v bytes="$size_bytes" -v dur="$duration_sec" '
        BEGIN { 
            if (dur > 0) print int((bytes * 8) / (dur * 1000))
            else print 0
        }'
}

# Check if file meets target specifications (skip logic)
is_compliant_format() {
    local file="$1"
    local format codec channels bitrate
    
    # Verify container compatibility  
    format=$(get_format_name "$file")
    case "$format" in
        *mp4*|*m4a*|*m4b*) ;;
        *) return 1 ;;
    esac
    
    # Verify codec
    codec=$(get_audio_codec "$file")
    [[ "$codec" == "aac" || "$codec" == "libfdk_aac" ]] || return 1
    
    # Verify mono
    channels=$(get_audio_channels "$file")
    [[ "$channels" == "1" ]] || return 1
    
    # Check if bitrate is at or below our target
    bitrate=$(calculate_bitrate_kbps "$file")
    
    # Get target bitrate for comparison
    local settings=$(get_encoder_settings "$ENCODER" "$QUALITY")
    local target_bitrate
    
    if [[ "$ENCODER" == "fdk" ]]; then
        target_bitrate="${settings%%:*}"  # Extract bitrate (e.g., "56k")
        target_bitrate="${target_bitrate%k}"  # Remove 'k' suffix
    elif [[ "$NATIVE_VBR" == "1" ]]; then
        # VBR - use estimated range upper bound
        case "$QUALITY" in
            low) target_bitrate=56 ;;     # ~48-56k range
            medium) target_bitrate=72 ;;  # ~64-72k range  
            high) target_bitrate=96 ;;    # ~80-96k range
        esac
    else
        # CBR
        target_bitrate="${settings%%:*}"
        target_bitrate="${target_bitrate%k}"
    fi
    
    # Skip if current bitrate <= target + 10% tolerance
    (( bitrate <= target_bitrate + target_bitrate/10 ))
}

get_encoder_settings() {
    local encoder="$1"
    local quality="$2"
    
    if [[ "$encoder" == "fdk" ]]; then
        echo "${FDK_PROFILES[$quality]:-${FDK_PROFILES[medium]}}"
    elif [[ "$NATIVE_VBR" == "1" ]]; then
        echo "${NATIVE_VBR_PROFILES[$quality]:-${NATIVE_VBR_PROFILES[medium]}}"
    else
        echo "${NATIVE_CBR_PROFILES[$quality]:-${NATIVE_CBR_PROFILES[medium]}}"
    fi
}

build_encode_command() {
    local input="$1"
    local output="$2"
    local settings="$3"
    
    IFS=':' read -r bitrate_or_quality cutoff <<< "$settings"
    
    local cmd=(ffmpeg -hide_banner -i "$input")
    
    # Sample mode - encode only first 30 seconds
    [[ "$SAMPLE_MODE" == "1" ]] && cmd+=(-t 30)
    
    # Audio encoding settings
    if [[ "$ENCODER" == "fdk" ]]; then
        cmd+=(
            -c:a libfdk_aac
            -profile:a aac_he
            -afterburner "$AFTERBURNER"
            -b:a "$bitrate_or_quality"
            -cutoff "$cutoff"
        )
    else
        cmd+=(
            -c:a aac
            -aac_coder twoloop  # Better quality for low bitrates
        )
        
        # VBR vs CBR for native AAC
        if [[ "$NATIVE_VBR" == "1" ]]; then
            cmd+=(-q:a "$bitrate_or_quality")  # VBR quality
        else
            cmd+=(-b:a "$bitrate_or_quality")  # CBR bitrate
        fi
        
        cmd+=(-cutoff "$cutoff")
    fi
    
    # Channel configuration
    [[ "$MONO_OUTPUT" == "1" ]] && cmd+=(-ac 1)
    
    # Metadata and output
    cmd+=(
        -map_metadata 0
        -map_chapters 0
        -movflags +faststart
        -y "$output"
    )
    
    echo "${cmd[@]}"
}

encode_file() {
    local input="$1"
    local base_name="${input%.*}"
    
    # Skip conditions - check first
    local output="${base_name}_${ENCODER}"
    [[ "$ENCODER" == "native" && "$NATIVE_VBR" == "1" ]] && output="${output}_vbr"
    [[ "$ENCODER" == "native" && "$NATIVE_VBR" == "0" ]] && output="${output}_cbr"
    [[ "$SAMPLE_MODE" == "1" ]] && output="${output}_sample"
    output="${output}_${QUALITY}.m4b"
    
    # Skip if output already exists
    if [[ -f "$output" ]]; then
        echo "Skipping (output exists): $(basename "$output")"
        return 0
    fi
    
    # Skip if input meets target specs
    if is_compliant_format "$input"; then
        echo "Skipping (already compliant): $input"
        return 0
    fi
    
    # Get encoder settings
    local settings=$(get_encoder_settings "$ENCODER" "$QUALITY")
    
    # Build command
    local cmd=$(build_encode_command "$input" "$output" "$settings")
    
    # Preview mode - just show command
    if [[ "$PREVIEW_MODE" == "1" ]]; then
        echo "Preview mode - Command to execute:"
        echo "$cmd"
        return 0
    fi
    
    # Setup logging
    mkdir -p "$LOG_DIR"
    echo "=== Encoding: $input ===" | tee -a "$LOG_FILE"
    local encoder_mode="$ENCODER"
    [[ "$ENCODER" == "native" ]] && encoder_mode="$ENCODER ($([[ "$NATIVE_VBR" == "1" ]] && echo "VBR" || echo "CBR"))"
    echo "Encoder: $encoder_mode ($QUALITY profile)" | tee -a "$LOG_FILE"
    [[ "$SAMPLE_MODE" == "1" ]] && echo "Sample mode: 30 seconds only" | tee -a "$LOG_FILE"
    
    # Display settings appropriately
    if [[ "$ENCODER" == "native" && "$NATIVE_VBR" == "1" ]]; then
        echo "Settings: VBR q=${settings%%:*} @ ${settings##*:}Hz" | tee -a "$LOG_FILE"
    else
        echo "Settings: CBR ${settings//:/ @ }Hz" | tee -a "$LOG_FILE"
    fi
    
    # Execute encoding with timing
    local start_time=$(date +%s)
    
    if $cmd 2>&1 | tee -a "$LOG_FILE"; then
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        
        # Human-friendly output
        if [[ $duration -lt 1 ]]; then
            echo "✓ Done (instant)" | tee -a "$LOG_FILE"
        elif [[ $duration -lt 60 ]]; then
            echo "✓ Done in ${duration}s" | tee -a "$LOG_FILE"
        else
            echo "✓ Done in $((duration / 60))m $((duration % 60))s" | tee -a "$LOG_FILE"
        fi
        
        # File size comparison
        if [[ -f "$input" ]] && [[ -f "$output" ]]; then
            local input_size=$(stat -f%z "$input" 2>/dev/null || stat -c%s "$input")
            local output_size=$(stat -f%z "$output" 2>/dev/null || stat -c%s "$output")
            local reduction=$(( 100 - (output_size * 100 / input_size) ))
            echo "Size reduction: ${reduction}%" | tee -a "$LOG_FILE"
        fi
    else
        echo "✗ Encoding failed" | tee -a "$LOG_FILE"
        return 1
    fi
}

# =============================================================================
# TEST CONFIGURATIONS - Easy comparison of encoding options
# =============================================================================

run_quality_tests() {
    local input="$1"
    
    echo "Running quality comparison tests..."
    echo "Input: $input"
    [[ "$SAMPLE_MODE" == "1" ]] && echo "Sample mode: 30-second samples only"
    echo
    
    # Test all quality levels for both encoders
    for encoder in fdk native; do
        ENCODER="$encoder"
        
        # Skip FDK if not available
        if [[ "$encoder" == "fdk" ]] && ! ffmpeg -hide_banner -encoders 2>/dev/null | grep -q "libfdk_aac"; then
            echo "Skipping FDK tests (not available)"
            continue
        fi
        
        if [[ "$encoder" == "native" ]]; then
            # Test both VBR and CBR for native
            for vbr_mode in 1 0; do
                NATIVE_VBR="$vbr_mode"
                local mode_name=$([[ "$vbr_mode" == "1" ]] && echo "VBR" || echo "CBR")
                
                for quality in low medium high; do
                    QUALITY="$quality"
                    echo "--- Testing: native $mode_name @ $quality ---"
                    encode_file "$input"
                    echo
                done
            done
        else
            # FDK only uses CBR
            for quality in low medium high; do
                QUALITY="$quality"
                echo "--- Testing: $encoder @ $quality ---"
                encode_file "$input"
                echo
            done
        fi
    done
}

# =============================================================================
# MAIN - Simple argument handling and execution
# =============================================================================

show_usage() {
    cat <<EOF
Usage: $0 [encoder] [quality] [input_file]  # Process single file
       $0 [encoder] [quality]                  # Process all audio files (batch)
       $0 test [input_file]                    # Run quality tests on specific file

Encoders:
  fdk     - libfdk_aac with HE-AAC profile (CBR)
  native  - FFmpeg native AAC with twoloop (VBR default)

Quality levels:
  low     - Maximum compression
            FDK CBR: 48k | Native VBR: ~48-56k | Native CBR: 56k
  medium  - Balanced [default]
            FDK CBR: 56k | Native VBR: ~64-72k | Native CBR: 72k
  high    - Premium quality
            FDK CBR: 64k | Native VBR: ~80-96k | Native CBR: 96k

Environment variables:
  AFTERBURNER=0|1  - FDK afterburner (default: 1)
  VBR=0|1          - Native AAC mode (default: 1=VBR, 0=CBR)
  PREVIEW=1        - Show command without executing
  SAMPLE=1         - Encode only first 30 seconds for testing

Examples:
  $0 native medium                      # Batch: encode all audio files
  $0 fdk low audiobook.mp3              # Single: encode specific file
  SAMPLE=1 $0 native medium             # Batch: 30-sec samples of all files
  $0 native high audiobook.m4a          # Single: Native AAC VBR q=1.0
  VBR=0 $0 native medium book.mp3       # Single: Native AAC CBR 72k
  SAMPLE=1 $0 test audiobook.mp3        # Test: all encoders on specific file
  PREVIEW=1 $0 fdk low test.mp3         # Preview: show command only
EOF
}

main() {
    # Handle help
    if [[ "${1:-}" == "-h" ]] || [[ "${1:-}" == "--help" ]]; then
        show_usage
        exit 0
    fi
    
    # Test mode - compare all options
    if [[ "${1:-}" == "test" ]]; then
        if [[ -z "${2:-}" ]]; then
            echo "Error: test mode requires input file" >&2
            show_usage
            exit 1
        fi
        check_requirements
        # Preserve SAMPLE_MODE if set by user
        local sample_setting="$SAMPLE_MODE"
        SAMPLE_MODE="$sample_setting"
        run_quality_tests "$2"
        exit 0
    fi
    
    # Validate encoder choice first
    if [[ "$ENCODER" != "fdk" ]] && [[ "$ENCODER" != "native" ]]; then
        echo "Error: encoder must be 'fdk' or 'native'" >&2
        show_usage
        exit 1
    fi
    
    check_requirements
    
    # Batch mode - process all audio files if no specific file given
    if [[ -z "$INPUT_FILE" ]]; then
        local -a audio_files=( *.m4b *.mp3 *.m4a )
        
        if [[ ${#audio_files[@]} -eq 0 ]] || [[ "${audio_files[0]}" == "*.m4b" ]]; then
            echo "No audio files found (*.m4b *.mp3 *.m4a)" >&2
            exit 1
        fi
        
        echo "Batch mode: Processing ${#audio_files[@]} file(s)"
        echo "Encoder: $ENCODER | Quality: $QUALITY"
        [[ "$ENCODER" == "native" ]] && echo "Mode: $([[ "$NATIVE_VBR" == "1" ]] && echo "VBR" || echo "CBR")"
        [[ "$SAMPLE_MODE" == "1" ]] && echo "Sample mode: 30-second samples only"
        echo
        
        for file in "${audio_files[@]}"; do
            if [[ -f "$file" ]]; then
                echo "[$(date +%H:%M:%S)] Processing: $file"
                encode_file "$file"
                echo
            fi
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

# Run if not sourced
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
