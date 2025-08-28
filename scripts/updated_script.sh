#!/bin/bash

# Absolute paths ensure we call the Homebrew build you expect.
# Overridable via env: FFMPEG_BIN=/custom/ffmpeg FFPROBE_BIN=/custom/ffprobe
FFMPEG_BIN=${FFMPEG_BIN:-/opt/homebrew/bin/ffmpeg}
FFPROBE_BIN=${FFPROBE_BIN:-/opt/homebrew/bin/ffprobe}

# Audio encoding toggles (override via env when calling the script)
# - Keeps your default HE-AACv1 profile and exposes Afterburner.
# - The single-colon ":" is the POSIX "set default" builtin; earlier "::<...>" disabled defaults.
: "${AUDIO_PROFILE:=aac_he}"     # {aac_low,aac_he,aac_he_v2,aac_ld,aac_eld}
: "${FDK_AFTERBURNER:=1}"        # 1=on, 0=off (only for libfdk_aac)

for input_file in *.m4b *.mp3 *.m4a; do
    if [ -f "$input_file" ]; then
        # Extract input properties once; "a:0" selects the first audio stream.
        # csv=p=0 simplifies parsing; guards below handle empties.
        current_bit_rate=$("$FFPROBE_BIN" -v error -select_streams a:0 -show_entries stream=bit_rate -of csv=p=0 "$input_file")
        current_sample_rate=$("$FFPROBE_BIN" -v error -select_streams a:0 -show_entries stream=sample_rate -of csv=p=0 "$input_file")
        current_channels=$("$FFPROBE_BIN" -v error -select_streams a:0 -show_entries stream=channels -of csv=p=0 "$input_file")

        # Preferred targets
        convert_bit_rate=64k
        convert_sample_rate=48000   # use numeric Hz to avoid unit ambiguity
        convert_channels=1

        # Only clear a target if current value is numeric and already at/below it.
        [[ "$current_bit_rate" =~ ^[0-9]+$ ]] && [ "$current_bit_rate" -le 64000 ] && convert_bit_rate=""
        [[ "$current_sample_rate" =~ ^[0-9]+$ ]] && [ "$current_sample_rate" -le 48000 ] && convert_sample_rate=""
        [[ "$current_channels"    =~ ^[0-9]+$ ]] && [ "$current_channels" -le 1 ] && convert_channels=""

        # Skip file if no conversion needed (avoids extra ffprobe calls and writes)
        if [ -z "$convert_bit_rate" ] && [ -z "$convert_sample_rate" ] && [ -z "$convert_channels" ]; then
            echo "Skipping $input_file (already meets preferred settings)"
            continue
        fi

        # Read tags for output path after deciding we will convert.
        artist=$("$FFPROBE_BIN" -v error -show_entries format_tags=artist -of csv=p=0 "$input_file")
        title=$("$FFPROBE_BIN" -v error -show_entries format_tags=title -of csv=p=0 "$input_file")

        output_dir="$(dirname "$input_file")/${artist:-Unknown Artist}"
        mkdir -p "$output_dir"
        output_file="${output_dir}/${artist:-Unknown Artist} - ${title:-Untitled}.m4b"

        # Choose encoder: prefer libfdk_aac (with afterburner), fallback to native aac.
        if "$FFMPEG_BIN" -hide_banner -encoders | grep -q 'libfdk_aac'; then
            audio_codec=( -c:a libfdk_aac -profile:a "$AUDIO_PROFILE" -afterburner "$FDK_AFTERBURNER" )
            encoder_name="libfdk_aac"
        else
            audio_codec=( -c:a aac -profile:a "$AUDIO_PROFILE" )
            encoder_name="aac"
        fi

        # Build the command in an array to avoid quoting bugs and "eval".
        cmd=( "$FFMPEG_BIN" -hide_banner -i "$input_file" -map 0:a -map 0:v? -c:v copy "${audio_codec[@]}" )
        [ -n "$convert_bit_rate" ]    && cmd+=( -b:a "$convert_bit_rate" )
        [ -n "$convert_sample_rate" ] && cmd+=( -ar "$convert_sample_rate" )
        [ -n "$convert_channels" ]    && cmd+=( -ac "$convert_channels" )
        cmd+=( -map_metadata 0 -map_chapters 0 -y "$output_file" )

        echo "Encoding profile: $AUDIO_PROFILE | Afterburner: $FDK_AFTERBURNER | Encoder: $encoder_name"
        if "${cmd[@]}"; then
            echo "Converted $input_file"
        else
            echo "FAILED $input_file" >&2
        fi
    fi
done
