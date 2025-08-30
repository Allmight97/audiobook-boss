#!/bin/bash
# AGENTS: This script is historical reference. The user ran manually it via terminal within target directory containing audio files to process single or multiple audbiobooks. 
# It can be used as reference to the uesrs preferred encoding logic. NOTE the user was ignorant of encoder profiles at time this script was written a few years ago.

for input_file in *.m4b *.mp3 *.m4a; do
    if [ -f "$input_file" ]; then
        # Extract current properties once and store in variables
        current_bit_rate=$(ffprobe -v error -select_streams a -show_entries stream=bit_rate -of default=noprint_wrappers=1:nokey=1 "$input_file")
        current_sample_rate=$(ffprobe -v error -select_streams a -show_entries stream=sample_rate -of default=noprint_wrappers=1:nokey=1 "$input_file")
        current_channels=$(ffprobe -v error -select_streams a -show_entries stream=channels -of default=noprint_wrappers=1:nokey=1 "$input_file")
        artist=$(ffprobe -v error -show_entries format_tags=artist -of default=noprint_wrappers=1:nokey=1 "$input_file")
        title=$(ffprobe -v error -show_entries format_tags=title -of default=noprint_wrappers=1:nokey=1 "$input_file")
        
        # Determine conversion requirements
        convert_bit_rate=64k
        convert_sample_rate=48k
        convert_channels=1
        
        [ "$current_bit_rate" -le 64000 ] && convert_bit_rate=""
        [ "$current_sample_rate" -le 48000 ] && convert_sample_rate=""
        [ "$current_channels" -le 1 ] && convert_channels=""
        
        # Skip file if no conversion needed
        if [ -z "$convert_bit_rate" ] && [ -z "$convert_sample_rate" ] && [ -z "$convert_channels" ]; then
            echo "Skipping $input_file (already meets preferred settings)"
            continue
        fi
        
        # Create output directory and file name
        output_dir="$(dirname "$input_file")/$artist"
        mkdir -p "$output_dir"
        output_file="${output_dir}/${artist} - ${title}.m4b"
        
        # Construct ffmpeg command
        #ffmpeg_command="ffmpeg -i \"$input_file\" -vn -c:a libfdk_aac -b:a $convert_bit_rate" #original command
        ffmpeg_command="ffmpeg -i \"$input_file\" -map 0:a -map 0:v? -c:a libfdk_aac -c:v copy -b:a $convert_bit_rate" #maps all audio and video (if exists)
        [ -n "$convert_sample_rate" ] && ffmpeg_command+=" -ar $convert_sample_rate"
        [ -n "$convert_channels" ] && ffmpeg_command+=" -ac $convert_channels"
        ffmpeg_command+=" -map_metadata 0 -map_chapters 0 -y \"$output_file\""
        
        # Execute the conversion
        eval "$ffmpeg_command"
        
        echo "Converted $input_file"
    fi
done
