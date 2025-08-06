#!/bin/bash
# Advanced FFmpeg Migration Analysis
# Detailed analysis of function signatures and patterns

echo "🧪 Advanced FFmpeg Migration Analysis"
echo "====================================="

AG="$HOME/.cargo/bin/ast-grep"

echo ""
echo "1. Function Definitions that need signature changes"
echo "-------------------------------------------------"
echo "Functions returning Result<Command>:"
$AG -p 'fn $func($$$) -> Result<Command>' src/
echo ""
echo "Functions with Command parameters:"
$AG -p 'fn $func($A: Command, $$$)' src/
echo ""

echo "2. Struct fields that will change"
echo "--------------------------------"
$AG -p 'struct $S { $$$ binary_path: $T, $$$ }' src/
echo ""

echo "3. Match expressions on process types"
echo "------------------------------------"
$AG -p 'match $expr { $$$ Child $$$ }' src/
echo ""

echo "4. Error handling patterns to update"
echo "-----------------------------------"
$AG -p 'FFmpegError::$variant($$$)' src/
echo ""

echo "5. Method calls on Command objects"
echo "---------------------------------"
$AG -p '$cmd.spawn($$$)' src/
$AG -p '$cmd.output($$$)' src/
echo ""

echo "6. Progress parsing regex patterns"
echo "---------------------------------"
$AG -p 'frame=' src/
$AG -p 'time=' src/
echo ""

echo "7. Temporary file patterns for concat"
echo "------------------------------------"
$AG -p 'concat_list' src/
$AG -p 'pipe:0' src/
echo ""

echo "8. Import statements to modify"
echo "-----------------------------"
$AG -p 'use std::process::$A' src/
echo ""

echo "9. Constants that may need updating"
echo "----------------------------------"
$AG -p 'const $NAME: &str = $V' src/ | grep -i ffmpeg
echo ""

echo "10. Test functions that need updating"
echo "-----------------------------------"
$AG -p '#[test] fn $func($$$) { $$$ Command $$$ }' src/
echo ""

echo ""
echo "📊 Detailed Analysis Complete!"
echo "🎯 This provides the exact patterns to search and replace during migration"
