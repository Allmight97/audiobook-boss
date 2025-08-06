# ast-grep Setup and Usage Guide for Audiobook-Boss

## Installation Complete ✅

**ast-grep** is now installed and ready for use in the FFmpeg-next migration.

### **Access Methods:**
```bash
# Full path (always works)
~/.cargo/bin/ast-grep

# Alias (set in current session)  
alias ag='~/.cargo/bin/ast-grep'
ag --version
```

### **Installation Verification:**
```bash
$ ~/.cargo/bin/ast-grep --version
ast-grep 0.39.3
```

## Key Migration Patterns

### **1. Finding FFmpeg Usage**
```bash
# All FFmpeg imports
ag -p 'use crate::ffmpeg' src/

# FFmpeg function calls
ag -p 'locate_ffmpeg($$$)' src/

# Command construction
ag -p 'Command::new($A)' src/
```

### **2. Function Signature Analysis**
```bash
# Functions returning Command
ag -p 'fn $func($$$) -> Result<Command>' src/

# Functions taking Command parameters  
ag -p 'fn $func($A: Command, $$$)' src/

# Method calls that will change
ag -p '$cmd.spawn($$$)' src/
ag -p '$cmd.output($$$)' src/
```

### **3. Pattern Replacement Examples**
```bash
# Find and replace (dry run first)
ag -p 'build_merge_command($$$)' --replace 'create_transcode_context($$$)' src/

# Update imports
ag -p 'use std::process::Command' --replace 'use ffmpeg_next::Transcoder' src/
```

## Migration Analysis Scripts

### **Basic Analysis Script**
`scripts/analyze_ffmpeg_migration.sh` - Identifies all code requiring changes

### **Advanced Analysis Script**  
`scripts/advanced_migration_analysis.sh` - Detailed pattern analysis

### **Usage:**
```bash
./scripts/analyze_ffmpeg_migration.sh
./scripts/advanced_migration_analysis.sh
```

## Key Findings from Analysis

### **Files Requiring Major Changes:**
1. **`src/ffmpeg/command.rs`** - Complete replacement needed
2. **`src/audio/media_pipeline.rs`** - Function signature updates  
3. **`src/audio/progress_monitor.rs`** - Process → callback conversion
4. **`src/commands/mod.rs`** - API call updates

### **Critical Patterns to Replace:**
- ✅ `Command::new()` → `ffmpeg::format::input()`
- ✅ `build_merge_command()` → New FFmpeg-next API
- ✅ `execute_ffmpeg_with_progress_context()` → Callback-based execution
- ✅ `locate_ffmpeg()` → Remove (handled by ffmpeg-next)

### **Import Changes:**
```rust
// BEFORE
use std::process::Command;
use crate::ffmpeg;

// AFTER  
use ffmpeg_next::{Transcoder, format};
use crate::ffmpeg::native;
```

## Migration Workflow with ast-grep

### **Phase 1: Analysis**
```bash
# Generate complete change list
./scripts/analyze_ffmpeg_migration.sh > migration_checklist.txt

# Find specific patterns
ag -p 'pattern_here' src/
```

### **Phase 2: Systematic Replacement**
```bash
# Replace function calls
ag -p 'old_function($$$)' --replace 'new_function($$$)' src/

# Update imports
ag -p 'use old::path' --replace 'use new::path' src/

# Verify changes
ag -p 'old_pattern' src/  # Should return empty
```

### **Phase 3: Validation**
```bash
# Ensure no old patterns remain
ag -p 'Command::new' src/
ag -p 'locate_ffmpeg' src/
ag -p 'build_merge_command' src/

# Check for new patterns
ag -p 'ffmpeg_next' src/
ag -p 'Transcoder' src/
```

## Advanced Queries for Migration

### **Finding Complex Patterns:**
```bash
# Nested function calls
ag -p '$obj.$method($inner.call($$$))' src/

# Error handling patterns
ag -p 'match $expr { Err($err) => $$$ }' src/

# Async function signatures  
ag -p 'async fn $name($$$) -> $ret { $$$ }' src/
```

### **Code Quality Analysis:**
```bash
# Functions over 50 lines (approximate)
ag -p 'fn $func($$$) { $body }' src/ | grep -A 50 'fn '

# Unwrap usage (should be avoided)
ag -p '$expr.unwrap()' src/

# TODO comments
ag -p '// TODO' src/
```

## Integration with Migration Process

### **Daily Migration Workflow:**
1. **Morning:** Run analysis scripts to see current state
2. **Work Session:** Use ast-grep to find specific patterns  
3. **Validation:** Verify changes don't break compilation
4. **Evening:** Run full analysis to track progress

### **Pre-Commit Checks:**
```bash
# Ensure no old patterns remain
./scripts/validate_migration_progress.sh

# Check for new required patterns
ag -p 'ffmpeg_next::' src/ | wc -l  # Should increase
ag -p 'Command::new' src/ | wc -l   # Should decrease
```

### **Testing Integration:**
```bash
# Find test functions that need updates
ag -p '#[test] fn $func($$$) { $$$ Command $$$ }' src/

# Find functions without tests
ag -p 'pub fn $func($$$)' src/ | grep -v test
```

## Benefits Demonstrated

### **Precision:**
- Finds exact patterns vs. regex false positives
- Context-aware search (understands Rust syntax)
- Safe replacement (won't break strings or comments)

### **Efficiency:**
- Fast tree-sitter based parsing
- Incremental analysis during development
- Scriptable for automation

### **Migration Safety:**
- Validates patterns before replacement
- Tracks progress systematically
- Prevents missed conversions

## Next Steps

1. **Use analysis scripts** to create detailed migration checklist
2. **Start with isolated modules** (test pattern replacement)
3. **Validate each change** with compilation checks
4. **Track progress** using ast-grep pattern counts
5. **Create new patterns** as migration progresses

---

## Quick Reference

### **Common Commands:**
```bash
# Find pattern
ag -p 'pattern' src/

# Find and replace (preview)
ag -p 'old' --replace 'new' src/

# Case insensitive  
ag -p 'pattern' -i src/

# Multiple patterns
ag -p 'pattern1' -p 'pattern2' src/
```

### **Pattern Syntax:**
- `$var` - Single node match
- `$$$` - Multiple nodes  
- `$$var$$` - Multiple nodes with capture
- `$A: Type` - Typed parameters

**ast-grep is now ready to accelerate our FFmpeg-next migration! 🚀**
