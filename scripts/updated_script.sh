#!/bin/bash
# Enhanced Audiobook Processing Script
# Implements HE-AAC v1 encoding with intelligent skipping, progress tracking, and parallel processing
# Author: Enhanced from original_reference_script.sh

set -euo pipefail  # Fail fast on errors, undefined variables, and pipe failures

# =============================================================================
# CONFIGURATION
# =============================================================================

# Encoding settings optimized for HE-AAC v1 (56-80k range)
readonly TARGET_BITRATE="64k"
readonly TARGET_SAMPLE_RATE="48000"
readonly TARGET_CHANNELS="1"
readonly AUDIO_PROFILE="aac_he"  # HE-AAC v1
readonly USE_AFTERBURNER="1"

# Processing settings
readonly MAX_PARALLEL_JOBS="2"
readonly SUPPORTED_FORMATS="*.m4b *.mp3 *.m4a"

# Logging settings
readonly LOG_LEVEL="${LOG_LEVEL:-INFO}"  # DEBUG, INFO, WARN, ERROR
readonly SHOW_PROGRESS="${SHOW_PROGRESS:-1}"

# =============================================================================
# GLOBAL VARIABLES
# =============================================================================

declare -g TOTAL_DURATION_SECONDS=0
declare -g PROCESSED_DURATION_SECONDS=0
declare -g TOTAL_FILES=0
declare -g PROCESSED_FILES=0
declare -g SKIPPED_FILES=0
declare -g FAILED_FILES=0
declare -g START_TIME
declare -A JOB_PIDS=()

# =============================================================================
# UTILITY FUNCTIONS
# =============================================================================

log() {
    local level="$1"
    shift
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    case "$level" in
        ERROR) echo "[$timestamp] ERROR: $*" >&2 ;;
        WARN)  echo "[$timestamp] WARN: $*" >&2 ;;
        INFO)  [[ "$LOG_LEVEL" =~ ^(DEBUG|INFO)$ ]] && echo "[$timestamp] INFO: $*" ;;
        DEBUG) [[ "$LOG_LEVEL" == "DEBUG" ]] && echo "[$timestamp] DEBUG: $*" ;;
    esac
}

# Cleanup function for signal handling
cleanup() {
    local exit_code=$?
    log "INFO" "Cleaning up..."
    
    # Kill any running background jobs
    for pid in "${JOB_PIDS[@]}"; do
        if kill -0 "$pid" 2>/dev/null; then
            log "DEBUG" "Terminating job PID $pid"
            kill -TERM "$pid" 2>/dev/null || true
        fi
    done
    
    # Wait for jobs to finish
    wait 2>/dev/null || true
    
    log "INFO" "Cleanup completed"
    exit $exit_code
}

# Set up signal handling
trap cleanup EXIT INT TERM

# =============================================================================
# VALIDATION FUNCTIONS  
# =============================================================================

check_dependencies() {
    log "INFO" "Checking dependencies..."
    
    # Check for ffmpeg
    if ! command -v ffmpeg >/dev/null 2>&1; then
        log "ERROR" "ffmpeg is not installed or not in PATH"
        log "ERROR" "Please install ffmpeg: https://ffmpeg.org/download.html"
        return 1
    fi
    
    # Check for ffprobe
    if ! command -v ffprobe >/dev/null 2>&1; then
        log "ERROR" "ffprobe is not installed or not in PATH"
        log "ERROR" "ffprobe is typically included with ffmpeg installation"
        return 1
    fi
    
    # Check for libfdk_aac support
    if ! ffmpeg -hide_banner -encoders 2>/dev/null | grep -q "libfdk_aac"; then
        log "ERROR" "libfdk_aac encoder is not available in your ffmpeg installation"
        log "ERROR" ""
        log "ERROR" "To install libfdk_aac on macOS:"
        log "ERROR" "  brew install ffmpeg --with-fdk-aac"
        log "ERROR" "  OR"
        log "ERROR" "  brew tap homebrew-ffmpeg/ffmpeg"
        log "ERROR" "  brew install homebrew-ffmpeg/ffmpeg/ffmpeg --with-fdk-aac"
        log "ERROR" ""
        log "ERROR" "On Linux (Ubuntu/Debian):"
        log "ERROR" "  sudo apt update"
        log "ERROR" "  sudo apt install libfdk-aac-dev"
        log "ERROR" "  # Then recompile ffmpeg with --enable-libfdk-aac"
        log "ERROR" ""
        log "ERROR" "For other systems, please consult your package manager or compile from source"
        return 1
    fi
    
    log "INFO" "All dependencies verified successfully"
    return 0
}

validate_audio_file() {
    local file="$1"
    
    # Check if file exists and is readable
    if [[ ! -f "$file" ]] || [[ ! -r "$file" ]]; then
        log "WARN" "File not accessible: $file"
        return 1
    fi
    
    # Basic format validation using ffprobe
    if ! ffprobe -v error -show_format "$file" >/dev/null 2>&1; then
        log "WARN" "Invalid or corrupted audio file: $file"
        return 1
    fi
    
    return 0
}

# Sanitize metadata for safe filename usage
sanitize_filename() {
    local input="$1"
    # Remove/replace problematic characters for cross-platform compatibility
    echo "$input" | sed 's/[<>:"/\\|?*]/_/g' | sed 's/[[:space:]]\+/ /g' | sed 's/^[[:space:]]*//;s/[[:space:]]*$//'
}

# =============================================================================
# AUDIO ANALYSIS FUNCTIONS
# =============================================================================

get_audio_properties() {
    local file="$1"
    local -n props=$2
    
    # Get all properties in a single ffprobe call for efficiency
    local probe_output
    probe_output=$(ffprobe -v error -select_streams a:0 \
        -show_entries stream=bit_rate,sample_rate,channels:format_tags=artist,title:format=duration \
        -of csv=p=0:s=, "$file" 2>/dev/null) || return 1
    
    # Parse comma-separated values
    IFS=',' read -r bit_rate sample_rate channels artist title duration <<< "$probe_output"
    
    props[bit_rate]=${bit_rate:-0}
    props[sample_rate]=${sample_rate:-0}
    props[channels]=${channels:-0}
    props[artist]=$(sanitize_filename "${artist:-Unknown Artist}")
    props[title]=$(sanitize_filename "${title:-$(basename "$file" | sed 's/\.[^.]*$//')}")
    props[duration]=${duration:-0}
    
    log "DEBUG" "Properties for $file: ${props[bit_rate]}bps, ${props[sample_rate]}Hz, ${props[channels]}ch, ${props[duration]}s"
    return 0
}

needs_conversion() {
    local -n props=$1
    
    # Check if any parameter exceeds targets
    local needs_bitrate_conv=0
    local needs_samplerate_conv=0
    local needs_channels_conv=0
    
    [[ ${props[bit_rate]} -gt $((${TARGET_BITRATE%k} * 1000)) ]] && needs_bitrate_conv=1
    [[ ${props[sample_rate]} -gt ${TARGET_SAMPLE_RATE} ]] && needs_samplerate_conv=1  
    [[ ${props[channels]} -gt ${TARGET_CHANNELS} ]] && needs_channels_conv=1
    
    if [[ $needs_bitrate_conv -eq 1 ]] || [[ $needs_samplerate_conv -eq 1 ]] || [[ $needs_channels_conv -eq 1 ]]; then
        log "DEBUG" "Conversion needed - bitrate:$needs_bitrate_conv samplerate:$needs_samplerate_conv channels:$needs_channels_conv"
        return 0  # Needs conversion
    fi
    
    log "DEBUG" "File already meets target specifications"
    return 1  # No conversion needed
}

output_file_exists() {
    local input_file="$1" 
    local -n props=$2
    
    local output_dir="$(dirname "$input_file")/${props[artist]}"
    local output_file="${output_dir}/${props[artist]} - ${props[title]}.m4b"
    
    if [[ -f "$output_file" ]]; then
        log "DEBUG" "Output file already exists: $output_file"
        return 0  # File exists
    fi
    
    return 1  # File doesn't exist
}

# =============================================================================
# PROGRESS TRACKING FUNCTIONS
# =============================================================================

update_progress() {
    local processed_seconds="$1"
    local file_duration="$2"
    local current_file="$3"
    
    # Update global progress
    PROCESSED_DURATION_SECONDS=$((PROCESSED_DURATION_SECONDS + processed_seconds))
    
    if [[ $SHOW_PROGRESS -eq 1 ]] && [[ $TOTAL_DURATION_SECONDS -gt 0 ]]; then
        local processed_minutes=$((PROCESSED_DURATION_SECONDS / 60))
        local total_minutes=$((TOTAL_DURATION_SECONDS / 60))
        local progress_percent=$((PROCESSED_DURATION_SECONDS * 100 / TOTAL_DURATION_SECONDS))
        
        # Calculate ETA
        local elapsed_seconds=$(($(date +%s) - START_TIME))
        local eta_seconds=0
        if [[ $PROCESSED_DURATION_SECONDS -gt 0 ]]; then
            eta_seconds=$(((TOTAL_DURATION_SECONDS - PROCESSED_DURATION_SECONDS) * elapsed_seconds / PROCESSED_DURATION_SECONDS))
        fi
        local eta_minutes=$((eta_seconds / 60))
        
        printf "\rProgress: %d/%d minutes (%d%%) | Files: %d/%d | ETA: %dm | Current: %s" \
            "$processed_minutes" "$total_minutes" "$progress_percent" \
            "$PROCESSED_FILES" "$TOTAL_FILES" "$eta_minutes" \
            "$(basename "$current_file")"
    fi
}

# =============================================================================
# PROCESSING FUNCTIONS
# =============================================================================

process_single_file() {
    local input_file="$1"
    local job_id="$2"
    
    log "DEBUG" "Processing file: $input_file (job $job_id)"
    
    # Validate file
    if ! validate_audio_file "$input_file"; then
        ((FAILED_FILES++))
        return 1
    fi
    
    # Get audio properties
    declare -A props
    if ! get_audio_properties "$input_file" props; then
        log "ERROR" "Failed to analyze file: $input_file"
        ((FAILED_FILES++))
        return 1
    fi
    
    # Check if conversion needed
    if ! needs_conversion props; then
        log "INFO" "Skipping $input_file (already meets target specifications)"
        ((SKIPPED_FILES++))
        update_progress "${props[duration]}" "${props[duration]}" "$input_file"
        return 0
    fi
    
    # Check if output already exists
    if output_file_exists "$input_file" props; then
        log "INFO" "Skipping $input_file (output already exists)"
        ((SKIPPED_FILES++))
        update_progress "${props[duration]}" "${props[duration]}" "$input_file"
        return 0
    fi
    
    # Create output directory
    local output_dir="$(dirname "$input_file")/${props[artist]}"
    if ! mkdir -p "$output_dir"; then
        log "ERROR" "Failed to create output directory: $output_dir"
        ((FAILED_FILES++))
        return 1
    fi
    
    local output_file="${output_dir}/${props[artist]} - ${props[title]}.m4b"
    
    # Build ffmpeg command safely (no eval)
    local -a ffmpeg_cmd=(
        ffmpeg
        -i "$input_file"
        -map 0:a
        -map 0:v?                    # Optional video stream (cover art)
        -c:a libfdk_aac
        -profile:a "$AUDIO_PROFILE"  # HE-AAC v1
        -afterburner "$USE_AFTERBURNER"
        -b:a "$TARGET_BITRATE"
        -ar "$TARGET_SAMPLE_RATE"
        -ac "$TARGET_CHANNELS"
        -c:v copy                    # Copy video stream as-is
        -map_metadata 0              # Preserve all metadata
        -map_chapters 0              # Preserve chapters
        -loglevel error              # Suppress verbose chapter output
        -progress pipe:1             # Enable progress monitoring
        -y                           # Overwrite output
        "$output_file"
    )
    
    log "INFO" "Converting: $(basename "$input_file") -> $(basename "$output_file")"
    log "DEBUG" "Command: ${ffmpeg_cmd[*]}"
    
    # Execute conversion with progress monitoring
    local start_time=$(date +%s)
    if "${ffmpeg_cmd[@]}" 2>/dev/null | while IFS= read -r line; do
        if [[ $line =~ ^out_time_ms=([0-9]+) ]]; then
            local current_ms="${BASH_REMATCH[1]}"
            local current_seconds=$((current_ms / 1000000))
            update_progress "$current_seconds" "${props[duration]}" "$input_file"
        fi
    done; then
        local end_time=$(date +%s)
        local processing_time=$((end_time - start_time))
        log "INFO" "Successfully converted $input_file (${processing_time}s)"
        ((PROCESSED_FILES++))
        return 0
    else
        log "ERROR" "Failed to convert $input_file"
        ((FAILED_FILES++))
        return 1
    fi
}

# Job management for parallel processing
wait_for_job_slot() {
    while [[ ${#JOB_PIDS[@]} -ge $MAX_PARALLEL_JOBS ]]; do
        local finished_jobs=()
        for pid in "${!JOB_PIDS[@]}"; do
            if ! kill -0 "$pid" 2>/dev/null; then
                finished_jobs+=("$pid")
            fi
        done
        
        # Remove finished jobs
        for pid in "${finished_jobs[@]}"; do
            unset JOB_PIDS["$pid"]
        done
        
        # If still at capacity, wait briefly
        [[ ${#JOB_PIDS[@]} -ge $MAX_PARALLEL_JOBS ]] && sleep 0.1
    done
}

# =============================================================================
# MAIN PROCESSING LOGIC
# =============================================================================

scan_and_calculate_totals() {
    log "INFO" "Scanning files and calculating total duration..."
    
    local file_count=0
    local total_seconds=0
    
    for pattern in $SUPPORTED_FORMATS; do
        for file in $pattern; do
            [[ ! -f "$file" ]] && continue
            
            if validate_audio_file "$file"; then
                declare -A props
                if get_audio_properties "$file" props; then
                    # Only count files that need processing
                    if needs_conversion props && ! output_file_exists "$file" props; then
                        ((file_count++))
                        total_seconds=$((total_seconds + ${props[duration]%.*}))  # Remove decimal part
                    fi
                fi
            fi
        done
    done
    
    TOTAL_FILES=$file_count
    TOTAL_DURATION_SECONDS=$total_seconds
    
    local total_minutes=$((total_seconds / 60))
    log "INFO" "Found $file_count files requiring processing (${total_minutes} minutes total)"
}

process_files() {
    log "INFO" "Starting parallel processing with $MAX_PARALLEL_JOBS concurrent jobs..."
    
    local job_counter=0
    
    for pattern in $SUPPORTED_FORMATS; do
        for input_file in $pattern; do
            [[ ! -f "$input_file" ]] && continue
            
            # Wait for available job slot
            wait_for_job_slot
            
            # Start background job
            ((job_counter++))
            process_single_file "$input_file" "$job_counter" &
            local job_pid=$!
            JOB_PIDS[$job_pid]="$input_file"
            
            log "DEBUG" "Started job $job_counter (PID: $job_pid) for file: $input_file"
        done
    done
    
    # Wait for all remaining jobs to complete
    log "INFO" "Waiting for all processing jobs to complete..."
    for pid in "${!JOB_PIDS[@]}"; do
        wait "$pid" || log "WARN" "Job PID $pid completed with errors"
    done
    
    # Clear completed jobs
    JOB_PIDS=()
}

print_summary() {
    echo ""  # New line after progress indicator
    log "INFO" "Processing completed!"
    log "INFO" "========================="
    log "INFO" "Total files processed: $PROCESSED_FILES"
    log "INFO" "Files skipped: $SKIPPED_FILES"  
    log "INFO" "Files failed: $FAILED_FILES"
    
    local total_time=$(($(date +%s) - START_TIME))
    local hours=$((total_time / 3600))
    local minutes=$(((total_time % 3600) / 60))
    local seconds=$((total_time % 60))
    
    if [[ $hours -gt 0 ]]; then
        log "INFO" "Total processing time: ${hours}h ${minutes}m ${seconds}s"
    elif [[ $minutes -gt 0 ]]; then
        log "INFO" "Total processing time: ${minutes}m ${seconds}s"
    else
        log "INFO" "Total processing time: ${seconds}s"
    fi
    
    if [[ $PROCESSED_FILES -gt 0 ]] && [[ $TOTAL_DURATION_SECONDS -gt 0 ]]; then
        local processed_minutes=$((TOTAL_DURATION_SECONDS / 60))
        local speed_ratio=$((TOTAL_DURATION_SECONDS / total_time))
        log "INFO" "Audio processed: ${processed_minutes} minutes (${speed_ratio}x realtime)"
    fi
}

# =============================================================================
# MAIN EXECUTION
# =============================================================================

main() {
    START_TIME=$(date +%s)
    
    log "INFO" "Enhanced Audiobook Processing Script v2.0"
    log "INFO" "=========================================="
    log "INFO" "Target: HE-AAC v1, ${TARGET_BITRATE}, ${TARGET_SAMPLE_RATE}Hz, ${TARGET_CHANNELS} channel(s)"
    log "INFO" "Parallel jobs: $MAX_PARALLEL_JOBS"
    log "INFO" ""
    
    # Dependency validation
    if ! check_dependencies; then
        log "ERROR" "Dependency check failed. Exiting."
        exit 1
    fi
    
    # Scan for files and calculate totals
    scan_and_calculate_totals
    
    if [[ $TOTAL_FILES -eq 0 ]]; then
        log "INFO" "No files found requiring processing. All files may already be optimized or processed."
        exit 0
    fi
    
    # Process files
    process_files
    
    # Print summary
    print_summary
    
    # Exit with appropriate code
    if [[ $FAILED_FILES -gt 0 ]]; then
        log "WARN" "Some files failed to process. Check logs above."
        exit 2
    fi
    
    log "INFO" "All processing completed successfully!"
    exit 0
}

# Execute main function if script is run directly (not sourced)
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi