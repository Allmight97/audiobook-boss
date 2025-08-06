#!/bin/bash
# Migration Progress Validation
# Tracks conversion from old to new patterns

echo "📊 FFmpeg Migration Progress Tracker"
echo "===================================="

AG="$HOME/.cargo/bin/ast-grep"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo ""
echo "🔍 OLD PATTERNS (should decrease over time):"
echo "--------------------------------------------"

old_command=$($AG -p 'Command::new($A)' src/ 2>/dev/null | wc -l | tr -d ' ')
old_build=$($AG -p 'build_merge_command($$$)' src/ 2>/dev/null | wc -l | tr -d ' ')
old_execute=$($AG -p 'execute_ffmpeg_with_progress_context($$$)' src/ 2>/dev/null | wc -l | tr -d ' ')
old_locate=$($AG -p 'locate_ffmpeg($$$)' src/ 2>/dev/null | wc -l | tr -d ' ')
old_imports=$($AG -p 'use std::process::Command' src/ 2>/dev/null | wc -l | tr -d ' ')

printf "Command::new() usage: ${RED}%d${NC}\n" $old_command
printf "build_merge_command() usage: ${RED}%d${NC}\n" $old_build
printf "execute_ffmpeg_with_progress_context() usage: ${RED}%d${NC}\n" $old_execute
printf "locate_ffmpeg() usage: ${RED}%d${NC}\n" $old_locate
printf "std::process::Command imports: ${RED}%d${NC}\n" $old_imports

old_total=$((old_command + old_build + old_execute + old_locate + old_imports))
printf "TOTAL OLD PATTERNS: ${RED}%d${NC}\n" $old_total

echo ""
echo "✨ NEW PATTERNS (should increase over time):"
echo "--------------------------------------------"

new_ffmpeg=$($AG -p 'ffmpeg_next' src/ 2>/dev/null | wc -l | tr -d ' ')
new_transcoder=$($AG -p 'Transcoder' src/ 2>/dev/null | wc -l | tr -d ' ')
new_processor=$($AG -p 'FFmpegProcessor' src/ 2>/dev/null | wc -l | tr -d ' ')
new_native=$($AG -p 'use crate::ffmpeg::native' src/ 2>/dev/null | wc -l | tr -d ' ')

printf "ffmpeg_next usage: ${GREEN}%d${NC}\n" $new_ffmpeg
printf "Transcoder usage: ${GREEN}%d${NC}\n" $new_transcoder
printf "FFmpegProcessor usage: ${GREEN}%d${NC}\n" $new_processor
printf "Native module imports: ${GREEN}%d${NC}\n" $new_native

new_total=$((new_ffmpeg + new_transcoder + new_processor + new_native))
printf "TOTAL NEW PATTERNS: ${GREEN}%d${NC}\n" $new_total

echo ""
echo "📈 MIGRATION PROGRESS:"
echo "---------------------"

if [ $old_total -eq 0 ] && [ $new_total -gt 0 ]; then
    printf "Status: ${GREEN}COMPLETE ✅${NC}\n"
    echo "🎉 Migration successfully completed!"
elif [ $new_total -gt 0 ] && [ $old_total -gt 0 ]; then
    progress=$((new_total * 100 / (old_total + new_total)))
    printf "Status: ${YELLOW}IN PROGRESS ⚠️  (%d%% converted)${NC}\n" $progress
    echo "📝 Continue replacing old patterns with new ones"
elif [ $new_total -eq 0 ] && [ $old_total -gt 0 ]; then
    printf "Status: ${RED}NOT STARTED ❌${NC}\n"
    echo "🚀 Ready to begin migration!"
else
    printf "Status: ${YELLOW}UNKNOWN 🤔${NC}\n"
    echo "❓ No patterns detected - check analysis scripts"
fi

echo ""
echo "🎯 NEXT ACTIONS:"
echo "---------------"

if [ $old_command -gt 0 ]; then
    echo "• Replace Command::new() with FFmpegProcessor::new()"
fi

if [ $old_build -gt 0 ]; then
    echo "• Update build_merge_command() calls"  
fi

if [ $old_execute -gt 0 ]; then
    echo "• Convert execute_ffmpeg_with_progress_context() to callbacks"
fi

if [ $old_locate -gt 0 ]; then
    echo "• Remove locate_ffmpeg() calls (handled by ffmpeg-next)"
fi

if [ $old_imports -gt 0 ]; then
    echo "• Update import statements to use ffmpeg-next"
fi

if [ $old_total -eq 0 ]; then
    echo "• ✅ All old patterns converted!"
    echo "• Run tests to verify functionality"
    echo "• Update documentation"
    echo "• Remove adapter functions"
fi

echo ""
echo "📋 VALIDATION COMMANDS:"
echo "-----------------------"
echo "Run tests: cargo test"
echo "Check compilation: cargo check"  
echo "Run clippy: cargo clippy"
echo "Full analysis: ./scripts/analyze_ffmpeg_migration.sh"

echo ""
echo "📊 Progress tracking complete!"
