#!/bin/zsh
# OPUS Audiobook Encoder (The "Next Gen" Engine)
# Tailored for Audiobookshelf + BookPlayer (iOS)
#
# Usage: ./shrink-opus.sh [-I DIR] [-O DIR]
# Toggles: PREVIEW=1 DRY=1 BITRATE=48k CHANNELS=0 JOBS=2
#
# - Input: MP3/M4A/M4B (Single files or subdirectories to merge)
# - Output: .m4b (MP4 container with Opus audio)
# - Quality: libopus @ 48k (default) - Stereo imaging preserved
# - Metadata: Preserves chapters, cover art, and standard tags

set -euo pipefail
setopt NULL_GLOB
setopt EXTENDED_GLOB

die() { echo "Error: $*" >&2; exit 1; }

# --- Configuration Defaults ---
PREVIEW="${PREVIEW:-0}"             # 1 = Encode 30s sample only
DRY="${DRY:-0}"                     # 1 = Show commands, don't run
BITRATE="${BITRATE:-48k}"           # Target bitrate (32k-64k is the sweet spot for Opus)
CHANNELS="${CHANNELS:-0}"           # 0 = Source, 1 = Mono, 2 = Stereo
JOBS="${JOBS:-2}"                   # Parallel jobs for single files
STATS_PERIOD="${STATS_PERIOD:-2}"

# Opus Specifics
OPUS_COMPRESSION="10"               # 0-10 (10 = Max quality/efficiency, slower)
OPUS_FRAME_DURATION="20"            # 20ms is standard for music/speech mix

# --- Dependency Check ---
check_prereqs() {
  if ! command -v ffmpeg >/dev/null 2>&1; then die "ffmpeg not found."; fi
  if ! ffmpeg -encoders 2>/dev/null | grep -q "libopus"; then
    die "Your ffmpeg does not support 'libopus'. Please install it (brew install ffmpeg)."
  fi
}

# --- Helpers ---

# Resolve channel flags (0=Source, 1=Mono, 2=Stereo)
resolve_channels() {
  case "${CHANNELS}" in
    0) echo "" ;;
    1|2) echo "-ac $CHANNELS" ;;
    *) die "CHANNELS must be 0 (source), 1, or 2" ;;
  esac
}

# Build Opus Encoder Flags
build_opus_flags() {
  local channel_flag
  channel_flag=$(resolve_channels)
  
  # -c:a libopus        : Use the Opus encoder
  # -b:a $BITRATE       : Target bitrate
  # -vbr on             : Variable Bitrate (Native Opus mode)
  # -compression_level  : CPU trade-off (10 = best quality)
  # -application audio  : 'audio' preserves music/stereo imaging better than 'voip'
  
  echo "$channel_flag -c:a libopus -b:a $BITRATE -vbr on -compression_level $OPUS_COMPRESSION -application audio"
}

# Extract Metadata (Cached)
typeset -gA META_CACHE
get_metadata() {
  local file="$1" key="$2"
  if [ -z "${META_CACHE[$file]:-}" ]; then
    # Cache basic stats: duration, bitrate, channels, artist
    local data
    data=$(ffprobe -v error -select_streams a:0 -show_entries \
      format=duration,bit_rate:format_tags=artist:stream=channels \
      -of default=noprint_wrappers=1:nokey=1 "$file" 2>/dev/null)
    META_CACHE[$file]="${data//$'\n'/|}"
  fi
  
  # Poor man's parsing of the piped cache string
  # Note: This relies on specific ffprobe output order. 
  # For robustness in a script this size, we re-probe specific keys if needed, 
  # but here we assume the cache is sufficient for logic.
  case "$key" in
    "channels") ffprobe -v error -select_streams a:0 -show_entries stream=channels -of default=noprint_wrappers=1:nokey=1 "$file" ;;
    "duration") ffprobe -v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$file" ;;
    "artist")   ffprobe -v error -show_entries format_tags=artist -of default=noprint_wrappers=1:nokey=1 "$file" ;;
  esac
}

# Directory Setup
setup_dirs() {
  INPUT_DIR="${INPUT_DIR:-$PWD}"
  OUTPUT_DIR="${OUTPUT_DIR:-$INPUT_DIR}"
  mkdir -p "$OUTPUT_DIR"
}

# --- Core Logic ---

process_merge() {
  local dir="$1"
  local name="$(basename "$dir")"
  local safe_name=$(echo "$name" | tr -cd '[:alnum:]._-')
  local out_file="$OUTPUT_DIR/${safe_name}.m4b"
  
  if [ -f "$out_file" ] && [ "$PREVIEW" -ne 1 ]; then echo "Skipping merge: $out_file exists"; return; fi

  # Gather Files
  local -a files
  files=("$dir"/*.mp3(N) "$dir"/*.m4a(N) "$dir"/*.m4b(N))
  files=("${(@on)files}") # Sort alphabetically
  
  if [ ${#files[@]} -eq 0 ]; then return; fi
  
  echo "Merging ${#files[@]} files in '$name' -> Opus M4B..."

  # Generate Inputs & Chapters
  local meta_file
  meta_file=$(mktemp)
  echo ";FFMETADATA1" > "$meta_file"
  
  local -a inputs
  local -a filter_str
  local idx=0
  local current_ms=0

  for f in "${files[@]}"; do
    inputs+=(-i "$f")
    filter_str+="[$idx:a:0]"
    
    # Calculate duration for chapters
    local dur
    dur=$(get_metadata "$f" "duration")
    local dur_ms=$(printf "%.0f" "$(($dur * 1000))")
    local title="$(basename "$f" | sed 's/\.[^.]*$//')"
    
    # Preview Mode Clipping
    if [ "$PREVIEW" -eq 1 ]; then dur_ms=30000; inputs+=(-t 30); fi

    echo "[CHAPTER]\nTIMEBASE=1/1000\nSTART=$current_ms\nEND=$((current_ms + dur_ms))\ntitle=$title" >> "$meta_file"
    
    current_ms=$((current_ms + dur_ms))
    idx=$((idx + 1))
    
    if [ "$PREVIEW" -eq 1 ] && [ $idx -ge 3 ]; then break; fi # Only do 3 files in preview
  done

  # Execute FFmpeg
  local cmd=(ffmpeg -hide_banner -stats -y)
  cmd+=("${inputs[@]}")
  cmd+=(-f ffmetadata -i "$meta_file")
  cmd+=(-filter_complex "${filter_str}concat=n=$idx:v=0:a=1[a]" -map "[a]")
  
  # Cover Art (Try to find cover.jpg/png or map from first file)
  if [ -f "$dir/cover.jpg" ]; then cmd+=(-i "$dir/cover.jpg" -map "$((idx+1)):0" -c:v copy -disposition:v:0 attached_pic); 
  elif [ -f "$dir/cover.png" ]; then cmd+=(-i "$dir/cover.png" -map "$((idx+1)):0" -c:v copy -disposition:v:0 attached_pic);
  else cmd+=(-map "0:v?" -c:v copy); fi

  # Opus Encoder Args
  local opus_args
  opus_args=$(build_opus_flags)
  cmd+=(${=opus_args})
  
  # Metadata & Output
  cmd+=(-map_chapters "$idx" -f mp4 -movflags +faststart "$out_file")

  if [ "$DRY" -eq 1 ]; then echo "${cmd[@]}"; else "${cmd[@]}"; fi
  rm -f "$meta_file"
}

process_single() {
  local file="$1"
  local base="$(basename "$file" | sed 's/\.[^.]*$//')"
  local out_file="$OUTPUT_DIR/${base}.m4b"

  # Skip if optimized
  if [ -f "$out_file" ] && [ "$PREVIEW" -ne 1 ]; then return; fi
  
  echo "Encoding '$base' -> Opus..."
  
  local preview_flag=""
  if [ "$PREVIEW" -eq 1 ]; then preview_flag="-t 60"; fi

  local opus_args
  opus_args=$(build_opus_flags)
  
  # FFmpeg Command
  # Note: -f mp4 is crucial to force Opus into the M4B container
  local cmd=(ffmpeg -hide_banner -stats -y -i "$file" $preview_flag)
  cmd+=(-map 0:a:0 -map "0:v?" -c:v copy)
  cmd+=(${=opus_args})
  cmd+=(-map_chapters 0 -f mp4 -movflags +faststart "$out_file")

  if [ "$DRY" -eq 1 ]; then echo "${cmd[@]}"; else "${cmd[@]}"; fi
}

# --- Main Execution ---

while getopts ":I:O:h" opt; do
  case "$opt" in
    I) INPUT_DIR="$OPTARG" ;;
    O) OUTPUT_DIR="$OPTARG" ;;
    h) echo "Usage: $0 [-I INPUT] [-O OUTPUT]"; exit 0 ;;
  esac
done

check_prereqs
setup_dirs

# 1. Find directories to merge
for dir in "$INPUT_DIR"/*(/); do
  # Check if dir contains audio
  if ls "$dir"/*.{mp3,m4a,m4b} >/dev/null 2>&1; then
    process_merge "$dir"
  fi
done

# 2. Find single files in root
for file in "$INPUT_DIR"/*.{mp3,m4a,m4b}(.); do
  process_single "$file" &
  
  # Job Limiter
  while [ $(jobs -r | wc -l) -ge "$JOBS" ]; do sleep 1; done
done

wait
echo "Done. Happy listening, JStar."