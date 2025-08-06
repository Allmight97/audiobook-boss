#!/bin/bash
# FFmpeg-next Migration Analysis using ast-grep
# This script identifies all code that needs to change during migration

echo "🔍 FFmpeg-next Migration Analysis"
echo "=================================="

AG="$HOME/.cargo/bin/ast-grep"

echo ""
echo "1. FFmpeg Module Imports (need to change)"
echo "----------------------------------------"
$AG -p 'use crate::ffmpeg' src/
echo ""

echo "2. Command::new() Usage (will be replaced)"
echo "-----------------------------------------"
$AG -p 'Command::new($A)' src/
echo ""

echo "3. FFmpeg Functions to Replace"
echo "----------------------------"
echo "build_merge_command usage:"
$AG -p 'build_merge_command($$$)' src/
echo ""
echo "execute_ffmpeg_with_progress_context usage:"
$AG -p 'execute_ffmpeg_with_progress_context($$$)' src/
echo ""

echo "4. FFmpeg Command Builder Pattern"
echo "--------------------------------"
$AG -p 'FFmpegCommand::$method($$$)' src/
echo ""

echo "5. Process Management (needs callback replacement)"
echo "------------------------------------------------"
$AG -p 'std::process::Child' src/
echo ""

echo "6. Progress Parsing (will become callbacks)"
echo "------------------------------------------"
$AG -p 'stderr.read_line($$$)' src/
echo ""

echo "7. Functions that Return Result<Command>"
echo "---------------------------------------"
$AG -p 'Result<Command>' src/
echo ""

echo "8. FFmpeg Version Checks"
echo "-----------------------"
$AG -p 'ffmpeg.*version' src/
echo ""

echo "9. Binary Location Code (will be removed)"
echo "-----------------------------------------"
$AG -p 'locate_ffmpeg($$$)' src/
echo ""

echo ""
echo "✅ Analysis Complete!"
echo "📊 Summary: This shows all code locations that need modification"
echo "🎯 Next: Use these patterns to guide systematic replacement"
