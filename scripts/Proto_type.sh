#!/bin/zsh
# Enhanced audiobook encoder Prototype encoding Engine.
# This script is used by the developer on their personal MacBook to process their personal audiobook library and is being actively used to test various audio encoders, encoding profiles, and settings that will inform decisions for the encoding engine for this application.
# This script is not intended to act in any way, shape, or form as a drop-in replacement for any functions within the Audiobook Boss application unless explicitly agreed upon between the developer and any human or AI agent collaborators. 
#
# Usage: ./shrink.sh [-I DIR] [-O DIR]
# Toggles: e.g. PREVIEW=1 DRY=1 ORDER_DEBUG=1 ENCODER=auto|fdk|apple FDK_VBR=4 BITRATE=59k CHANNELS=1 PROBESIZE=5M ANALYZE_DURATION=5M STATS_PERIOD=2 JOBS=1 MERGE_MODE=auto|separate|flatten
#
# Processes MP3s in subdirs → single M4B with chapters.
# Encodes individual files in current dir → optimized M4B.
# Prefers libfdk_aac HE-AAC VBR, falls back to aac_at CVBR.
# Preserves chapters, metadata, and cover art.
# Channel layout follows source unless CHANNELS override is set.

set -euo pipefail
set +x
setopt NULL_GLOB
setopt EXTENDED_GLOB

die() {
  echo "Error: $*" >&2
  exit 1
}

# Toggles
PREVIEW="${PREVIEW:-0}"
ORDER_DEBUG="${ORDER_DEBUG:-0}"
DRY="${DRY:-0}"       # DRY RUN: show planned operations without encoding
FDK_VBR="${FDK_VBR:-3}"        # libfdk_aac VBR quality: 1-5 (default is 3 which is ~60k)
BITRATE="${BITRATE:-64k}"       # used for aac_at CVBR targets 
CHANNELS="${CHANNELS:-}"        # optional channel override: empty=source, 1=mono, 2=stereo
ENCODER="${ENCODER:-auto}"   # auto|fdk|apple encoder choice
PROBESIZE="${PROBESIZE:-5M}"
ANALYZE_DURATION="${ANALYZE_DURATION:-5M}"
STATS_PERIOD="${STATS_PERIOD:-2}"
JOBS="${JOBS:-2}"
MERGE_MODE="${MERGE_MODE:-auto}" # auto|separate|flatten
FFMPEG_ENCODERS_CACHE=""

resolve_channels() {
  case "${CHANNELS:-}" in
    "")
      ;; # use source channels
    1|2)
      echo "-ac $CHANNELS"
      ;;
    *)
      die "CHANNELS must be empty, 1, or 2"
      ;;
  esac
}

resolve_threads() {
  local threads="${THREADS:-0}"
  if [[ -z "$threads" ]]; then
    echo 0
    return
  fi
  if [[ "$threads" == <-> ]] && [ "$threads" -ge 0 ]; then
    echo "$threads"
  else
    die "THREADS must be a non-negative integer"
  fi
}

ffmpeg_has_encoder() {
  local encoder="$1"
  if [ -z "${FFMPEG_ENCODERS_CACHE:-}" ]; then
    FFMPEG_ENCODERS_CACHE="$(ffmpeg -hide_banner -v error -encoders)"
  fi
  print -r -- "$FFMPEG_ENCODERS_CACHE" | grep -qE "\\b${encoder}\\b"
}

build_fdk_flags() {
  local channels="$1"
  shift
  local channel_flags=("$@")
  local -a args
  args=("${channel_flags[@]}" -c:a libfdk_aac -profile:a aac_he -vbr "$FDK_VBR" -afterburner 1)
  local desc="FDK HE-AAC (aac_he) VBR q=$FDK_VBR (afterburner)"
  print -r -- "${(j: :)args}|$desc"
}

build_apple_flags() {
  local channels="$1"
  shift
  local channel_flags=("$@")
  local -a args
  args=("${channel_flags[@]}" -c:a aac_at -aac_at_mode cvbr -b:a "$BITRATE")
  local desc="Apple aac_at CVBR $BITRATE"
  print -r -- "${(j: :)args}|$desc"
}

# Default locations
SCRIPT_PATH="${0:A}"
SCRIPT_DIR="${SCRIPT_PATH:h}"
INPUT_DIR="."
OUTPUT_DIR="$SCRIPT_DIR"

while getopts ":I:O:h" opt; do
  case "$opt" in
    I) INPUT_DIR="$OPTARG" ;;
    O) OUTPUT_DIR="$OPTARG" ;;
    h)
      echo "Usage: $0 [-I INPUT_DIR] [-O OUTPUT_DIR]"
      echo "  -I DIR        Process audio files located in DIR"
      echo "  -O DIR        Write outputs under DIR (default: script directory)"
      echo "  BITRATE=59k   Override Apple CVBR target (and fallback bitrate)"
      echo "  ENCODER=auto|fdk|apple selects encoder (default auto)"
      echo "    FDK forces HE-AAC (aac_he); Apple CVBR lets ffmpeg pick profile"
      echo "    Sample rate and stereo/mono are preserved from source"
      exit 0
      ;;
    \?)
      echo "Unknown option: -$OPTARG" >&2
      exit 2
      ;;
    :)
      echo "Option -$OPTARG requires an argument." >&2
      exit 2
      ;;
  esac
done
shift $((OPTIND - 1))

INPUT_DIR="${INPUT_DIR/#\~/$HOME}"
OUTPUT_DIR="${OUTPUT_DIR/#\~/$HOME}"

if [ ! -d "$INPUT_DIR" ]; then
  die "input directory does not exist: $INPUT_DIR"
fi

if [ ! -d "$OUTPUT_DIR" ]; then
  die "output directory does not exist: $OUTPUT_DIR"
fi
INPUT_DIR="$INPUT_DIR:A"
OUTPUT_DIR="$OUTPUT_DIR:A"
cd -- "$INPUT_DIR"

# Shared ffmpeg flag arrays (DRY)
FF_GLOBAL=(
  -hide_banner -stats -stats_period "$STATS_PERIOD" -loglevel warning -nostdin
  -threads $(resolve_threads) -probesize "$PROBESIZE" -analyzeduration "$ANALYZE_DURATION"
  -fflags +fastseek+genpts
)
FF_POST=(
  -map_metadata 0 -movflags +faststart
)

typeset -ga WORKLIST
typeset -gA WORK_MERGE_FILES
typeset -gi MERGE_TASK_COUNT=0
typeset -gi MERGE_PROCESSED=0
typeset -gi FILE_TASK_COUNT=0
typeset -ga RUNNING_PIDS=()
typeset -gi JOB_FAILURES=0
typeset -gA JOB_LABEL JOB_SOURCE
typeset -gi JOB_LAUNCHED=0 JOB_COMPLETED=0

# Global temp tracking and cleanup (avoids referencing locals in EXIT trap)
TEMP_FILES=()
typeset -gA META_DURATION META_DURATION_SEX META_SAMPLE_RATE META_CHANNELS META_BIT_RATE META_ARTIST META_CODEC META_CACHE_READY
cleanup_temp_files() {
  local f
  if [ ${#TEMP_FILES[@]} -gt 0 ]; then
    for f in "${TEMP_FILES[@]}"; do
      [ -n "${f:-}" ] && rm -f "$f" 2>/dev/null || true
    done
  fi
}
trap cleanup_temp_files EXIT

ensure_dir() {
  local dir="$1"
  [ -z "$dir" ] && die "ensure_dir called with empty path"
  mkdir -p "$dir" || die "Failed to create directory: $dir"
}

# Helper: print a fully quoted command line
print_quoted() {
  local -a cmd
  cmd=("$@")
  print -r -- ${(j: :)${(q)cmd}}
}

preview_args() {
  if [ "${PREVIEW:-0}" = "1" ]; then
    echo "-t 30"
  fi
}

execute_ffmpeg() {
  local -a before_post=()
  local -a after_post=()
  local section="before"
  local arg

  for arg in "$@"; do
    if [ "$arg" = "--POST--" ]; then
      section="after"
      continue
    fi

    if [ "$section" = "before" ]; then
      before_post+=("$arg")
    else
      after_post+=("$arg")
    fi
  done

  if [ "${DRY:-0}" != "1" ]; then
    ffmpeg "${FF_GLOBAL[@]}" "${before_post[@]}" "${FF_POST[@]}" "${after_post[@]}"
  else
    print_quoted ffmpeg "${FF_GLOBAL[@]}" "${before_post[@]}" "${FF_POST[@]}" "${after_post[@]}"
  fi
}

check_prereqs() {
  if ! command -v ffmpeg >/dev/null 2>&1; then
    echo "Error: ffmpeg not found. Install via Homebrew: brew install ffmpeg" >&2
    exit 1
  fi
  if ! command -v ffprobe >/dev/null 2>&1; then
    echo "Error: ffprobe not found. Install via Homebrew: brew install ffmpeg" >&2
    exit 1
  fi
local version_line
if version_line=$(ffmpeg -v quiet -hide_banner -version | head -n1); then
  echo "ffmpeg version: $version_line"
else
  echo "Warning: unable to read ffmpeg version" >&2
fi
}

# Helper function to extract metadata from audio files
prime_metadata_cache() {
  local file="$1"
  local cached="${META_CACHE_READY["$file"]:-0}"
  if [ "$cached" = "1" ]; then
    return
  fi

  local duration="0" format_bit_rate="0" stream_bit_rate="0" sample_rate="0" channels="0" artist="" codec_name=""
  local output="" key value

  if output=$(ffprobe -v error -select_streams a:0 -show_entries \
    format=duration,bit_rate:format_tags=artist:stream=sample_rate,channels,bit_rate,codec_name \
    -of flat=s=_ "$file" 2>/dev/null); then
    while IFS='=' read -r key value; do
      value="${value%\r}"
      value="${value%\n}"
      value="${value#\"}"
      value="${value%\"}"
      case "$key" in
        format_duration)
          duration="$value"
          ;;
        format_bit_rate)
          format_bit_rate="$value"
          ;;
        format_tags_artist)
          artist="$value"
          ;;
        streams_stream_0_sample_rate)
          sample_rate="$value"
          ;;
        streams_stream_0_channels)
          channels="$value"
          ;;
        streams_stream_0_bit_rate)
          stream_bit_rate="$value"
          ;;
        streams_stream_0_codec_name)
          codec_name="$value"
          ;;
      esac
    done <<EOF
$output
EOF
  else
    echo "Warning: ffprobe metadata read failed for $file" >&2
  fi

  local bit_rate="$stream_bit_rate"
  if [ -z "$bit_rate" ] || [ "$bit_rate" = "0" ] || [ "$bit_rate" = "N/A" ]; then
    bit_rate="$format_bit_rate"
  fi

  if [[ "$bit_rate" = <-> ]]; then
    if [ "$bit_rate" -le 0 ]; then
      echo "Warning: suspicious audio bitrate '$bit_rate' for $file" >&2
      bit_rate=0
    fi
  else
    if [ -n "$bit_rate" ] && [ "$bit_rate" != "N/A" ]; then
      echo "Warning: suspicious audio bitrate '$bit_rate' for $file" >&2
    fi
    bit_rate=0
  fi

  local duration_raw="${duration:-0}"
  local duration_int="${duration_raw%%.*}"
  if [ -z "$duration_int" ]; then
    duration_int=0
  fi
  if ! [[ "$duration_int" = <-> ]]; then
    duration_int=0
  fi

  local duration_sex="unknown"
  if [[ "$duration_int" = <-> ]]; then
    local hours=$(( duration_int / 3600 ))
    local minutes=$(( (duration_int % 3600) / 60 ))
    local seconds=$(( duration_int % 60 ))
    duration_sex=$(printf '%02d:%02d:%02d' "$hours" "$minutes" "$seconds")
  fi

  if ! [[ "$sample_rate" = <-> ]] || [ "$sample_rate" -le 0 ]; then
    sample_rate=0
  fi
  if ! [[ "$channels" = <-> ]] || [ "$channels" -le 0 ]; then
    channels=0
  fi
  if [ -z "$codec_name" ] || [ "$codec_name" = "N/A" ]; then
    codec_name="unknown"
  fi

  META_DURATION["$file"]="$duration_raw"
  META_DURATION_SEX["$file"]="$duration_sex"
  META_SAMPLE_RATE["$file"]="${sample_rate:-0}"
  META_CHANNELS["$file"]="${channels:-0}"
  META_BIT_RATE["$file"]="${bit_rate:-0}"
  META_ARTIST["$file"]="${artist:-}"
  META_CODEC["$file"]="${codec_name:-}"
  META_CACHE_READY["$file"]=1
}

get_metadata() {
  local file="$1"
  local key="$2"
  prime_metadata_cache "$file"

  case "$key" in
    "artist")
      echo "${META_ARTIST["$file"]:-}"
      ;;
    "duration")
      echo "${META_DURATION["$file"]:-0}"
      ;;
    "duration_sexagesimal")
      echo "${META_DURATION_SEX["$file"]:-unknown}"
      ;;
    "sample_rate")
      echo "${META_SAMPLE_RATE["$file"]:-0}"
      ;;
    "channels")
      echo "${META_CHANNELS["$file"]:-0}"
      ;;
    "bit_rate")
      echo "${META_BIT_RATE["$file"]:-0}"
      ;;
    "codec")
      echo "${META_CODEC["$file"]:-}"
      ;;
    *)
      echo "Unknown metadata key: $key" >&2
      return 1
      ;;
  esac
}

# Helper function to sanitize names for filesystem safety
sanitize_name() {
  local name="$1"
  local sanitized

  sanitized="$(echo "$name" | sed 's/[^a-zA-Z0-9 ._-]/_/g' | sed 's/__*/_/g' | sed 's/^_//;s/_$//')"

  # Final safety check - if empty after sanitization, use Unknown
  if [ -z "$sanitized" ]; then
    sanitized="Unknown"
  fi

  echo "$sanitized"
}

build_output_dir() {
  local artist="$1"
  local safe_artist=$(sanitize_name "$artist")
  local dir="$OUTPUT_DIR/$safe_artist"
  ensure_dir "$dir"
  echo "$dir"
}

build_worklist() {
  WORKLIST=()
  WORK_MERGE_FILES=()
  MERGE_TASK_COUNT=0
  FILE_TASK_COUNT=0

  local -a subdirs=()
  local dir
  for dir in ./*(/); do
    dir="${dir#./}"
    [ -d "$dir" ] || continue
    subdirs+=("$dir")
  done

  subdirs=("${(@on)subdirs}")

  local -a eligible_subdirs=()
  for dir in "${subdirs[@]}"; do
    local -a mp3_files=()
    while IFS= read -r -d '' mp3_file; do
      mp3_files+=("$mp3_file")
    done < <(find "$dir" -type f -iname "*.mp3" -print0)
    mp3_files=("${(@on)mp3_files}")
    if [ ${#mp3_files[@]} -gt 0 ]; then
      WORK_MERGE_FILES["${dir}"]="${(F)mp3_files}"
      eligible_subdirs+=("$dir")
    fi
  done

  local root_has_files=0
  local file
  local matches=(*.m4b *.mp3 *.m4a *.M4B *.MP3 *.M4A)
  if [ ${#matches[@]} -gt 0 ]; then
    for file in ${matches[@]}; do
      [[ -e "$file" ]] || continue
      case "$file" in
        *.mp3|*.MP3) root_has_files=1 ;;
      esac
      WORKLIST+=("file:${file}")
      ((++FILE_TASK_COUNT))
    done
  fi

  local flatten=0
  if [ "$root_has_files" -eq 0 ] && [ "$MERGE_MODE" != "separate" ] && [ ${#eligible_subdirs[@]} -gt 0 ]; then
    case "$MERGE_MODE" in
      flatten)
        flatten=1
        ;;
      auto)
        flatten=1
        ;;
    esac
  fi

  if [ $flatten -eq 1 ]; then
    local -a all_mp3=()
    for dir in "${eligible_subdirs[@]}"; do
      local serialized="${WORK_MERGE_FILES["${dir}"]}"
      [ -n "$serialized" ] || continue
      local -a mp3_files=( ${(f)serialized} )
      (( ${#mp3_files[@]} )) || continue
      all_mp3+=("${mp3_files[@]}")
      unset "WORK_MERGE_FILES[$dir]"
    done
    if [ ${#all_mp3[@]} -gt 0 ]; then
      WORK_MERGE_FILES["."]="${(F)all_mp3}"
      WORKLIST+=("merge:.")
      ((++MERGE_TASK_COUNT))
    fi
  else
    for dir in "${eligible_subdirs[@]}"; do
      WORKLIST+=("merge:${dir}")
      ((++MERGE_TASK_COUNT))
    done
  fi

  if [ ${#WORKLIST[@]} -eq 0 ]; then
    echo "No input files (*.m4b/*.mp3/*.m4a) found in current directory or subdirectories." >&2
    exit 0
  fi
}

# Encoder selection (returns encoder config as space-separated values)
choose_encoder() {
  local has_fdk=0
  local has_aac_at=0
  if ffmpeg_has_encoder "libfdk_aac"; then
    has_fdk=1
  fi
  if ffmpeg_has_encoder "aac_at"; then
    has_aac_at=1
  fi

  local channel_flags
  channel_flags=($(resolve_channels))

  # Determine channel count to respect explicit overrides
  # If CHANNELS override is set, use it; otherwise default to 2 (stereo)
  # This will be refined per-file based on detected channels
  local default_channels="${CHANNELS:-2}"

  case "$ENCODER" in
    auto)
      if [ "$has_fdk" -eq 1 ]; then
        build_fdk_flags "$default_channels" "${channel_flags[@]}"
      else
    echo "Warning: libfdk_aac encoder not found." >&2
        printf "Proceed with Apple aac_at CVBR %s fallback? [y/N]: " "$BITRATE" > /dev/tty
        local ans=""
        if ! read -r ans < /dev/tty; then
          ans=""
        fi
        case "$ans" in
          y|Y|yes|YES)
            build_apple_flags "$default_channels" "${channel_flags[@]}"
            ;;
          *)
            die "libfdk_aac unavailable"
            ;;
        esac
      fi
      ;;
    fdk)
      if [ "$has_fdk" -eq 1 ]; then
        build_fdk_flags "$default_channels" "${channel_flags[@]}"
      else
        die "ENCODER=fdk but libfdk_aac is not available"
      fi
      ;;
    apple)
      if [ "$has_aac_at" -eq 1 ]; then
        build_apple_flags "$default_channels" "${channel_flags[@]}"
      else
        die "ENCODER=apple but aac_at encoder is unavailable"
      fi
      ;;
    *)
      die "unknown ENCODER mode '$ENCODER'. Expected auto|fdk|apple"
      ;;
  esac
}

process_merge_task() {
  local top_dir="$1"
  local serialized="${WORK_MERGE_FILES["${top_dir}"]}"
  local -a mp3_files
  mp3_files=( ${(f)serialized} )
  [ ${#mp3_files[@]} -gt 0 ] || return

  local is_root_merge=0
  if [ "$top_dir" = "." ]; then
    is_root_merge=1
  fi

  local display_dir="$top_dir"
  if [ $is_root_merge -eq 1 ]; then
    display_dir="${PWD##*/}"
  fi

  echo "Found ${#mp3_files[@]} MP3 files in '${display_dir}' - merging and encoding into single audiobook..."

  local dir_name
  if [ $is_root_merge -eq 1 ]; then
    dir_name="${PWD##*/}"
  else
    dir_name="$(basename "${top_dir%/}")"
  fi
  local safe_name=$(sanitize_name "$dir_name")

  local first_mp3="${mp3_files[1]}"
  local artist=$(get_metadata "$first_mp3" "artist")
  if [ -z "${artist:-}" ] || [ "$artist" = "N/A" ]; then
    artist="Unknown"
  fi
  local safe_artist=$(sanitize_name "$artist")

  local out_dir
  out_dir=$(build_output_dir "$safe_artist")
  local output_file="${out_dir}/${safe_name}.m4b"

  if [ "${PREVIEW:-0}" != "1" ] && [ -f "$output_file" ]; then
    echo "  Skipping merge+encode: $output_file already exists"
    return
  fi

  if [ "${ORDER_DEBUG}" = "1" ]; then
    local debug_list_file="${out_dir}/${safe_name}.order.txt"
    : > "$debug_list_file"
    for p in "${mp3_files[@]}"; do
      printf "%s\n" "$INPUT_DIR/$p" >> "$debug_list_file"
    done
    echo "  [DEBUG] Wrote ordered list to: $debug_list_file"
  fi

  local chapter_metadata=$(mktemp)
  TEMP_FILES+=("$chapter_metadata")
  local current_time=0

  echo ";FFMETADATA1" > "$chapter_metadata"

  local filter_inputs=""
  local ff_inputs=()
  local idx=0
  for mp3_file in "${mp3_files[@]}"; do
    ff_inputs+=( -i "$mp3_file" )
    filter_inputs+="[${idx}:a:0]"
    idx=$((idx + 1))

    local duration=$(get_metadata "$mp3_file" "duration")
    local duration_ms=0
    if [[ -n "${duration:-}" ]]; then
      if [[ "$duration" == <-> ]]; then
        duration_ms=$(( duration * 1000 ))
      elif [[ "$duration" == <->.<-> ]]; then
        local duration_whole="${duration%%.*}"
        local duration_frac="${duration#*.}"
        duration_frac="${duration_frac%%[^0-9]*}"
        local frac_ms="${duration_frac}" 
        if [ -n "$frac_ms" ]; then
          if [ ${#frac_ms} -gt 3 ]; then
            frac_ms="${frac_ms:0:3}"
          fi
          while [ ${#frac_ms} -lt 3 ]; do
            frac_ms="${frac_ms}0"
          done
        else
          frac_ms="000"
        fi
        duration_ms=$(( duration_whole * 1000 + ${frac_ms:-0} ))
      fi
    fi

    local title_base="$(basename "$mp3_file")"
    title_base="${title_base%.*}"
    local chapter_title=$(sanitize_name "$title_base")
    echo "" >> "$chapter_metadata"
    echo "[CHAPTER]" >> "$chapter_metadata"
    echo "TIMEBASE=1/1000" >> "$chapter_metadata"
    echo "START=${current_time}" >> "$chapter_metadata"
    echo "END=$((current_time + duration_ms))" >> "$chapter_metadata"
    echo "title=$chapter_title" >> "$chapter_metadata"

    current_time=$((current_time + duration_ms))
  done

  local -a time_args_local
  time_args_local=($(preview_args))

  # Determine channel count for merged output
  # Use CHANNELS override if set, otherwise detect from first MP3
  local merge_channels="${CHANNELS:-}"
  if [ -z "$merge_channels" ]; then
    merge_channels=$(get_metadata "$first_mp3" "channels")
  fi

  # Rebuild encoder args for this merge task with channel-aware settings
  local merge_enc_config
  case "$ENCODER" in
    auto|fdk)
      merge_enc_config=$(build_fdk_flags "$merge_channels" $(resolve_channels))
      ;;
    apple)
      merge_enc_config=$(build_apple_flags "$merge_channels" $(resolve_channels))
      ;;
  esac

  local merge_enc_args_string="${merge_enc_config%|*}"
  local merge_enc_args=(${=merge_enc_args_string})
  local merge_enc_desc="${merge_enc_config#*|}"

  if [ "${DRY}" != "1" ]; then
    echo "  Encoding merged audiobook to: $output_file"
    echo "  Encoder: $merge_enc_desc"

    execute_ffmpeg \
      "${ff_inputs[@]}" \
      -f ffmetadata -i "$chapter_metadata" \
      ${time_args_local[@]+"${time_args_local[@]}"} \
      -filter_complex "${filter_inputs}concat=n=${#mp3_files[@]}:v=0:a=1[a]" \
      -map "[a]" -map '0:v:0?' -c:v copy \
      ${merge_enc_args[@]} \
      --POST-- -map_chapters ${#mp3_files[@]} \
      -y "$output_file"

    if [ "${DEBUG:-0}" = "1" ]; then
      local debug_ffmeta_file="${out_dir}/${safe_name}.chapters.ffmeta"
      cp "$chapter_metadata" "$debug_ffmeta_file" 2>/dev/null || true
      echo "  [DEBUG] Wrote chapters ffmetadata to: $debug_ffmeta_file"
    fi

    echo "  ✓ Merged and encoded: $output_file"
  else
    echo "  [DRY] Would encode merged audiobook to: $output_file"
    echo "  Encoder: $merge_enc_desc"
    execute_ffmpeg \
      "${ff_inputs[@]}" \
      -f ffmetadata -i "$chapter_metadata" \
      ${time_args_local[@]+"${time_args_local[@]}"} \
      -filter_complex "${filter_inputs}concat=n=${#mp3_files[@]}:v=0:a=1[a]" \
      -map "[a]" -map '0:v:0?' -c:v copy \
      ${merge_enc_args[@]} \
      --POST-- -map_chapters ${#mp3_files[@]} \
      -y "$output_file"
  fi
}

current_job_limit() {
  local raw="${JOBS:-1}"
  local -i limit
  if [[ "$raw" = <-> ]]; then
    limit="$raw"
  else
    limit=1
  fi
  if [ "$limit" -le 0 ]; then
    limit=1
  fi
  echo "$limit"
}

remove_running_pid() {
  local target="$1"
  local -a kept=()
  local pid
  for pid in "${RUNNING_PIDS[@]}"; do
    if [ -n "${pid:-}" ] && [ "$pid" != "$target" ]; then
      kept+=("$pid")
    fi
  done
  if [ ${#kept[@]} -gt 0 ]; then
    RUNNING_PIDS=("${kept[@]}")
  else
    RUNNING_PIDS=()
  fi
}

reap_finished_job() {
  local pid
  local -i exit_status=0
  local limit
  limit=$(current_job_limit)
  for pid in "${RUNNING_PIDS[@]}"; do
    [ -n "${pid:-}" ] || continue
    if ! kill -0 "$pid" 2>/dev/null; then
      if wait "$pid"; then
        exit_status=0
      else
        exit_status=$?
      fi
      remove_running_pid "$pid"
      (( ++JOB_COMPLETED ))
      if [ $exit_status -ne 0 ]; then
        (( ++JOB_FAILURES ))
        print -ru2 -- "${JOB_LABEL["$pid"]:-PID $pid} (PID $pid) FAILED (exit $exit_status) | active ${#RUNNING_PIDS[@]}/$(current_job_limit)"
      else
        print -ru2 -- "${JOB_LABEL["$pid"]:-PID $pid} (PID $pid) done | active ${#RUNNING_PIDS[@]}/$(current_job_limit)"
      fi
      return
    fi
  done

  pid="${RUNNING_PIDS[1]:-}"
  [ -n "${pid:-}" ] || return
  if wait "$pid"; then
    exit_status=0
  else
    exit_status=$?
  fi
  remove_running_pid "$pid"
  (( ++JOB_COMPLETED ))
  if [ $exit_status -ne 0 ]; then
    (( ++JOB_FAILURES ))
    print -ru2 -- "${JOB_LABEL["$pid"]:-PID $pid} (PID $pid) FAILED (exit $exit_status) | active ${#RUNNING_PIDS[@]}/$(current_job_limit)"
  else
    print -ru2 -- "${JOB_LABEL["$pid"]:-PID $pid} (PID $pid) done | active ${#RUNNING_PIDS[@]}/$(current_job_limit)"
  fi
}

pwait() {
  local limit
  limit=$(current_job_limit)
  while [ ${#RUNNING_PIDS[@]} -ge "$limit" ]; do
    reap_finished_job
  done
}

wait_for_remaining_jobs() {
  while [ ${#RUNNING_PIDS[@]} -gt 0 ]; do
    reap_finished_job
  done
}

# Main execution starts here
check_prereqs
build_worklist

if [ "${DRY:-0}" = "1" ]; then
  echo "[DRY] Queued $MERGE_TASK_COUNT merge task(s) and $FILE_TASK_COUNT single-file task(s)."
else
  echo "Queued $MERGE_TASK_COUNT merge task(s) and $FILE_TASK_COUNT single-file task(s)."
fi

# Get encoder configuration
encoder_config=$(choose_encoder)
enc_args_string="${encoder_config%|*}"
enc_args=(${=enc_args_string})
ENC_DESC="${encoder_config#*|}"

if [ "${PREVIEW}" = "1" ]; then
  # Preview mode: encode first 30s for quick spot-checks; bypass skip logic.
  echo "Preview mode enabled: encoding first 30 seconds"
fi

for task in "${WORKLIST[@]}"; do
  case "$task" in
    merge:*)
      top_dir="${task#merge:}"
      process_merge_task "$top_dir"
      (( ++MERGE_PROCESSED ))
      ;;
    file:*)
      input_file="${task#file:}"
      [[ -e "$input_file" ]] || continue
      pwait

      duration=$(get_metadata "$input_file" "duration")
      duration_str=$(get_metadata "$input_file" "duration_sexagesimal")
      sr=$(get_metadata "$input_file" "sample_rate")
      ch=$(get_metadata "$input_file" "channels")
      br=$(get_metadata "$input_file" "bit_rate")
      codec=$(get_metadata "$input_file" "codec")

      # Rebuild encoder args for this file with channel-aware settings
      # Passing channel data keeps overrides aligned with per-file detection
      local file_enc_config
      local file_enc_args_string
      local file_enc_args
      local file_enc_desc

      case "$ENCODER" in
        auto|fdk)
          file_enc_config=$(build_fdk_flags "$ch" $(resolve_channels))
          ;;
        apple)
          file_enc_config=$(build_apple_flags "$ch" $(resolve_channels))
          ;;
      esac

      file_enc_args_string="${file_enc_config%|*}"
      file_enc_args=(${=file_enc_args_string})
      file_enc_desc="${file_enc_config#*|}"

      # Skip when already within optimized mono/stereo thresholds
      if [ "${PREVIEW:-0}" != "1" ]; then
        if [[ "${ch:-}" = <-> && "${br:-}" = <-> ]] \
          && (( br > 0 && ((ch == 1 && br <= 64000) || (ch == 2 && br <= 80000)) )); then
          _skip_chan_desc="unknown-channel"
          case "$ch" in
            1) _skip_chan_desc="mono" ;;
            2) _skip_chan_desc="stereo" ;;
            *) _skip_chan_desc="${ch}-ch" ;;
          esac
          (( _skip_kb = (br + 500) / 1000 ))
          _skip_br="${_skip_kb} k"
          _skip_codec="${codec:-unknown}"
          if [ -z "$_skip_codec" ] || [ "$_skip_codec" = "N/A" ]; then
            _skip_codec="unknown"
          fi
          _skip_codec=${_skip_codec:u}
          echo "Skipping $input_file (already optimized: $_skip_chan_desc $_skip_br $_skip_codec)"
          continue
        fi
      fi

      base="${input_file##*/}"
      base="${base%.*}"
      artist=$(get_metadata "$input_file" "artist")
      if [ -z "${artist:-}" ] || [ "$artist" = "N/A" ]; then
        artist="Unknown"
      fi

      safe_artist=$(sanitize_name "$artist")
      out_dir=$(build_output_dir "$safe_artist")
      output_file="${out_dir}/${base}.m4b"

      if [ "${DRY:-0}" != "1" ]; then
        echo "Converting: $input_file"
        echo "  Duration: $duration_str | Channels: $ch | Encoder: $file_enc_desc"
        echo "  Output: $output_file"
        echo "  Progress: (watch 'size' and 'speed' below)"

        (
          local -a preview
          preview=($(preview_args))
          execute_ffmpeg \
            -i "$input_file" \
            ${preview[@]+"${preview[@]}"} \
            -map 0:a:0 -map '0:v?' -c:v copy \
            ${file_enc_args[@]} \
            --POST-- -map_chapters 0 \
            -y "$output_file"
          echo "  ✓ Complete"
        ) &
        job_pid=$!
        RUNNING_PIDS+=("$job_pid")
        (( ++JOB_LAUNCHED ))
        JOB_LABEL["$job_pid"]="Job $JOB_LAUNCHED/$FILE_TASK_COUNT"
        JOB_SOURCE["$job_pid"]="$input_file"
        print -ru2 -- "${JOB_LABEL["$job_pid"]}: $input_file (PID $job_pid) running | active ${#RUNNING_PIDS[@]}/$(current_job_limit)"

      else
        echo "  [DRY] Would convert: $input_file to $output_file"
        echo "  Channels: $ch | Encoder: $file_enc_desc"
        execute_ffmpeg \
          -i "$input_file" \
          ${preview[@]+"${preview[@]}"} \
          -map 0:a:0 -map '0:v?' -c:v copy \
          ${file_enc_args[@]} \
          --POST-- -map_chapters 0 \
          -y "$output_file"
      fi
      ;;
  esac
done
wait_for_remaining_jobs

if [ $MERGE_TASK_COUNT -gt 0 ]; then
  echo "Processed $MERGE_PROCESSED of $MERGE_TASK_COUNT merge task(s)."
fi

echo "Completed $JOB_COMPLETED of $JOB_LAUNCHED encode job(s)."

if [ $JOB_FAILURES -gt 0 ]; then
  echo "Encountered $JOB_FAILURES failed encode task(s)." >&2
  exit 1
fi
