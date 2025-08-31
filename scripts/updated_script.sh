#!/usr/bin/env bash

# Purpose: Transcode audiobook files to .m4b using libfdk_aac HE-AAC v1 with afterburner, size-focused.
# - Output: always .m4b
# - Quality: HE-AAC v1, afterburner ON, VBR default 5 (1-6 supported)
# - Sample rate: pass-through (never set -ar)
# - Channels: downmix to mono only if input > 1 ch
# - Metadata/chapters/cover art: preserved (map metadata/chapters, copy art streams)
# - Skip policy: if output exists, skip; also skip if input is already compliant m4b (AAC, mono, ~≤64k)
# - Progress: single line X/TOTAL minutes (and %) via ffmpeg -progress key-value output
# - Parallelization: optional up to 2 concurrent jobs
#
# Usage:
#   ./updated_script.sh                 # sequential, default VBR=5
#   ABB_VBR=5 ./updated_script.sh       # test VBR 5
#   ABB_JOBS=2 ./updated_script.sh      # run up to 2 files in parallel
#   ABB_PREVIEW=1 ./updated_script.sh   # enable 180s preview
#
# Notes:
# - Requires ffmpeg built with libfdk_aac. The script will fail fast with guidance if missing.
# - We do not merge multiple MP3 files; each input processed independently.

set -Eeuo pipefail
IFS=$'\n\t'

# Respect nullglob to avoid iterating literal globs when no files match
shopt -s nullglob

# Configuration knobs (can be overridden via environment)
ABB_VBR="${ABB_VBR:-5}"              # 5 (default) range 1-6
ABB_AFTERBURNER="${ABB_AFTERBURNER:-1}" # 1=on (default), 0=off
ABB_PREVIEW="${ABB_PREVIEW:-0}"      # 1=enable 180s preview, 0=off
ABB_JOBS="${ABB_JOBS:-1}"            # 1 (default) or 2 (max)

if [[ "$ABB_JOBS" != "1" && "$ABB_JOBS" != "2" ]]; then
  echo "ABB_JOBS must be 1 or 2; got '$ABB_JOBS'" >&2
  exit 2
fi

# Validate VBR knob
if ! [[ "$ABB_VBR" =~ ^[1-6]$ ]]; then
  echo "ABB_VBR must be an integer 1-6; got '$ABB_VBR' — defaulting to 5" >&2
  ABB_VBR="5"
fi

# Validate afterburner
if [[ "$ABB_AFTERBURNER" != "0" && "$ABB_AFTERBURNER" != "1" ]]; then
  echo "ABB_AFTERBURNER must be 0 or 1; got '$ABB_AFTERBURNER' — defaulting to 1" >&2
  ABB_AFTERBURNER="1"
fi

# Validate preview
if [[ "$ABB_PREVIEW" != "0" && "$ABB_PREVIEW" != "1" ]]; then
  echo "ABB_PREVIEW must be 0 or 1; got '$ABB_PREVIEW' — defaulting to 0" >&2
  ABB_PREVIEW="0"
fi



# Preflight checks
require_cmd() { command -v "$1" >/dev/null 2>&1 || { echo "Missing required command: $1" >&2; exit 127; }; }
require_cmd ffmpeg
require_cmd ffprobe

# Verify FDK is available
if ! ffmpeg -hide_banner -encoders 2>/dev/null | grep -q "libfdk_aac"; then
  cat >&2 <<'EOF'
ERROR: libfdk_aac encoder not available in your ffmpeg build.
On macOS, install an ffmpeg with FDK AAC, for example via Homebrew:
  brew tap homebrew-ffmpeg/ffmpeg
  brew install homebrew-ffmpeg/ffmpeg/ffmpeg --with-fdk-aac
Note: Options can change over time; ensure your ffmpeg lists 'libfdk_aac' in 'ffmpeg -encoders'.
Aborting.
EOF
  exit 1
fi

# Extra safety: verify ffmpeg actually uses libfdk_aac when requested (no silent fallback)
if ! ffmpeg -hide_banner -loglevel info -f lavfi -t 0.1 -i anullsrc=r=44100:cl=mono -c:a libfdk_aac -f null - 2>&1 | grep -q "libfdk_aac"; then
  echo "ERROR: ffmpeg did not report using libfdk_aac when requested. Aborting." >&2
  exit 1
fi

# Helpers
sec_to_minutes_floor() { # $1 seconds -> integer minutes (floor)
  awk -v s="$1" 'BEGIN { if (s==""||s=="N/A") {print 0} else {print int(s/60)} }'
}

sec_to_minutes_ceil() { # $1 seconds -> integer minutes (ceil, min 1 for any positive duration)
  awk -v s="$1" 'BEGIN { if (s==""||s=="N/A") {print 0} else {m=s/60; c=(m==int(m)?m:int(m)+1); if (c<1 && s>0) c=1; print c} }'
}

get_format_duration_sec() { # $1 input file
  ffprobe -v error -select_streams a:0 -show_entries format=duration -of default=nw=1:nk=1 -- "$1" 2>/dev/null || echo ""
}

get_audio_channels() { # $1 input file
  ffprobe -v error -select_streams a:0 -show_entries stream=channels -of default=nw=1:nk=1 -- "$1" 2>/dev/null || echo ""
}

get_format_name() { # $1 input file (container format names)
  ffprobe -v error -show_entries format=format_name -of default=nw=1:nk=1 -- "$1" 2>/dev/null | head -n1
}

get_audio_codec_name() { # $1 input file
  ffprobe -v error -select_streams a:0 -show_entries stream=codec_name -of default=nw=1:nk=1 -- "$1" 2>/dev/null | head -n1
}

get_tag() { # $1 file, $2 tag key
  ffprobe -v error -show_entries "format_tags=$2" -of default=nw=1:nk=1 -- "$1" 2>/dev/null | head -n1
}

sanitize_path_component() { # replace slashes and control chars
  tr -d '\0' | sed -E 's#[/\\]+#-#g; s#[:*?"<>|]#-#g; s#^[[:space:]]+|[[:space:]]+$##g; s#[[:space:]]+# #g'
}

estimate_avg_bitrate_kbps() { # $1 file -> integer kbps
  # Use container size and duration to estimate avg bitrate when bit_rate is N/A (works for CBR and VBR roughly)
  local size_bytes duration
  size_bytes=$(stat -f %z -- "$1" 2>/dev/null || stat -c %s -- "$1" 2>/dev/null || echo 0)
  duration=$(ffprobe -v error -show_entries format=duration -of default=nw=1:nk=1 -- "$1" 2>/dev/null || echo 0)
  awk -v bytes="$size_bytes" -v dur="$duration" 'BEGIN { if (dur<=0) {print 0} else {print int((bytes*8)/(dur*1000))} }'
}

is_compliant_m4b() { # $1 file -> 0 if compliant (AAC mono ~<=64k), 1 otherwise
  local fmt codec ch kbps
  fmt=$(get_format_name "$1")
  case "$fmt" in
    *mp4*|*m4a*|*m4b*) : ;; # OK container family
    *) return 1 ;;
  esac
  codec=$(get_audio_codec_name "$1")
  [[ "$codec" == "aac" || "$codec" == "libfdk_aac" ]] || return 1
  ch=$(get_audio_channels "$1")
  [[ "$ch" == "1" ]] || return 1
  kbps=$(estimate_avg_bitrate_kbps "$1")
  # allow a small headroom above 64 to account for container overhead (e.g., 68 kbps)
  if awk -v k="$kbps" 'BEGIN { exit (k <= 68 ? 0 : 1) }'; then
    return 0
  else
    return 1
  fi
}

process_one() { # $1 input file
  local input="$1"

  # Probe tags early for output path
  local artist title
  artist=$(get_tag "$input" artist || true)
  title=$(get_tag "$input" title || true)
  if [[ -z "$artist" ]]; then
    artist=$(get_tag "$input" album_artist || true)
  fi
  if [[ -z "$title" ]]; then
    title=$(get_tag "$input" album || true)
  fi
  if [[ -z "$artist" ]]; then
    artist="Unknown Artist"
  fi
  if [[ -z "$title" ]]; then
    title="$(basename "${input}")"
  fi
  local artist_s title_s
  artist_s=$(printf '%s' "$artist" | sanitize_path_component)
  title_s=$(printf '%s' "$title" | sanitize_path_component)

  local out_dir out_file tmp_file
  out_dir="$(dirname -- "$input")/${artist_s}"
  mkdir -p -- "$out_dir"
  local suffix=""
  if [[ "$ABB_PREVIEW" == "1" ]]; then
    suffix="-preview"
  fi
  out_file="${out_dir}/${artist_s} - ${title_s}${suffix}.m4b"
  tmp_file="${out_file%.m4b}.partial.m4b"

  if [[ -e "$out_file" ]]; then
    echo "Skipping (exists): $out_file"
    return 0
  fi

  # Optional skip if already compliant m4b
  if is_compliant_m4b "$input"; then
    echo "Skipping (already compliant m4b): $input"
    return 0
  fi

  # Compute duration and minutes for progress
  local dur_s total_min
  dur_s=$(get_format_duration_sec "$input")
  total_min=$(sec_to_minutes_ceil "$dur_s")

  # Channel handling: only downmix if needed
  local in_ch
  in_ch=$(get_audio_channels "$input")

  # Build ffmpeg command (array form). Preserve metadata/chapters. Copy art streams.
  # Map audio: copy all audio streams from input to keep parity with original script intent.
  # However, all audio streams will be re-encoded to HE-AAC v1; art/video streams are copied.
  local -a cmd
  cmd=(ffmpeg -hide_banner -loglevel info -y -i "$input")

  # Check if input has video streams (for cover art)
  has_video=$(ffprobe -v error -select_streams v -show_streams -- "$input" 2>/dev/null | grep -q 'codec_type=video' && echo 1 || echo 0)

  # Map audio
  cmd+=( -map 0:a )

  # Map video (cover art) only if present
  if [[ "$has_video" == "1" ]]; then
    cmd+=( -map 0:v -c:v copy )
  fi

  # Audio encoding params
  cmd+=( -c:a libfdk_aac -profile:a aac_he -afterburner "$ABB_AFTERBURNER" )
  cmd+=( -vbr "$ABB_VBR" )

  # Preview mode limits output to 180 seconds
  if [[ "$ABB_PREVIEW" == "1" ]]; then
    cmd+=( -t 180 )
  fi

  # Downmix only if input has more than 1 channel
  if [[ "$in_ch" =~ ^[0-9]+$ ]] && (( in_ch > 1 )); then
    cmd+=( -ac 1 )
  fi

  # Sample rate pass-through: do not set -ar

  # Preserve metadata and chapters
  cmd+=( -map_metadata 0 -map_chapters 0 )

  cmd+=( "$tmp_file" )

  # Run ffmpeg normally; errors will go to stderr
  {
    "${cmd[@]}"
  } || {
    echo
    echo "ERROR converting: $input" >&2
    rm -f -- "$tmp_file"
    return 1
  }

  # Atomic move into place
  mv -f -- "$tmp_file" "$out_file"
  echo "Done: $(basename -- "$out_file")"
}

# Gather inputs (same patterns as original script)
inputs=( *.m4b *.mp3 *.m4a )

if (( ${#inputs[@]} == 0 )); then
  echo "No input files found matching: *.m4b *.mp3 *.m4a"
  exit 0
fi

# Parallel execution (up to 2)
if [[ "$ABB_JOBS" == "2" ]]; then
  # Simple two-worker queue without external deps
  pids=()
  for f in "${inputs[@]}"; do
    (
      process_one "$f"
    ) &
    pids+=("$!")
    # Limit concurrency to 2
    if (( ${#pids[@]} >= 2 )); then
      wait "${pids[0]}" || true
      pids=("${pids[@]:1}")
    fi
  done
  # Wait remaining
  for pid in "${pids[@]}"; do
    wait "$pid" || true
  done
else
  # Sequential
  for f in "${inputs[@]}"; do
    process_one "$f"
  done
fi

echo "All done."
