# Dependency Management Guide for Audiobook Boss

## Primal Terms

**Dependency**: An external library or crate that your project relies on to function.

**`Cargo.toml`**: The manifest file for Rust projects, specifying metadata and dependencies with version constraints.

**`Cargo.lock`**: A file that records the exact versions of dependencies used, ensuring consistent builds across environments.

**Semantic Versioning (SemVer)**: A versioning scheme using MAJOR.MINOR.PATCH format where:
- `^1.0` allows `1.x.x` but not `2.0.0`
- `~1.0.0` allows `1.0.x` but not `1.1.0`
- `=1.0.0` allows exactly `1.0.0`

**Transitive Dependency**: A dependency required by another dependency, not directly by your project.

**Direct Dependency**: A dependency explicitly declared in your `Cargo.toml`.

**Feature Flags**: Optional functionality that can be enabled/disabled in dependencies.

## Key Commands for Managing Dependencies

### Analysis Commands
```bash
# View complete dependency tree
cargo tree

# View only direct dependencies
cargo tree --depth 1

# Focus on specific package
cargo tree -p package_name

# Find duplicate versions
cargo tree --duplicates

# Check for outdated dependencies
cargo outdated
```

### Update Commands
```bash
# Update all dependencies to latest compatible versions
cargo update

# Update specific dependency
cargo update -p package_name

# Update to latest versions (may break compatibility)
cargo update --aggressive
```

### Verification Commands
```bash
# Ensure project builds after updates
cargo build

# Run tests to verify compatibility
cargo test

# Check for linting issues
cargo clippy -- -D warnings

# Format code
cargo fmt --all -- --check
```

## Understanding Your Project's Dependencies

### Current Dependency Profile
**Direct Dependencies**: 13 packages
**Total Packages**: 548 packages
**Main Contributors**:
- Tauri ecosystem: ~200-300 packages
- ffmpeg-next ecosystem: ~100-150 packages
- Other transitive dependencies: ~100-200 packages

### Direct Dependencies Analysis
```toml
[dependencies]
anyhow v1.0.100          # Error handling - ✅ Essential
env_logger v0.11.8       # Logging - ✅ Essential
ffmpeg-next v8.0.0       # Audio processing - ✅ Core functionality
lofty v0.22.4            # Metadata handling - ✅ Core functionality
log v0.4.28              # Logging - ✅ Essential
serde v1.0.228           # Serialization - ✅ Essential
serde_json v1.0.145     # JSON handling - ✅ Essential
tauri v2.8.5             # Desktop framework - ✅ Core functionality
tauri-plugin-dialog v2.4.0    # File dialogs - ✅ Essential
tauri-plugin-opener v2.5.0    # File opening - ✅ Essential
thiserror v2.0.17        # Error handling - ✅ Essential
tokio v1.48.0            # Async runtime - ✅ Essential
uuid v1.18.1             # UUID generation - ✅ Essential
```

### Feature Flags Analysis
```toml
# Tokio - Appropriate for audio processing
tokio = { version = "1.0", features = ["full"] }

# Tauri - Includes necessary desktop features
tauri = { version = "2", features = [] }

# ffmpeg-next - Includes required audio processing features
ffmpeg-next = "8.0"
```

## Dependency Health Assessment

### Duplicate Analysis
**Status**: ✅ **HEALTHY** - Minimal duplicates detected

**Packages with 3+ versions**:
- `phf_shared` (3 versions: 0.8.0, 0.10.0, 0.11.3)
- `phf_generator` (3 versions: 0.8.0, 0.10.0, 0.11.3)
- `phf` (3 versions: 0.8.0, 0.10.0, 0.11.3)
- `getrandom` (3 versions: 0.1.16, 0.2.16, 0.3.3)
- `bitflags` (3 versions: 1.3.2, 2.9.4, 2.9.4)

**Why These Duplicates Exist**:
- **PHF (Perfect Hash Function)**: Used by Tauri's HTML parser (`kuchikiki`) and CSS parser (`cssparser`) for performance-critical lookups
- **getrandom**: Different versions required by different `rand` versions used by different crates
- **bitflags**: Different major versions used by different parts of the stack

### Bloat Assessment
**Status**: ✅ **NO BLOAT DETECTED**

**What Would Be Bloat**:
- Multiple HTTP clients (`reqwest`, `ureq`, `hyper`)
- Multiple async runtimes (`tokio`, `async-std`)
- Multiple serialization libraries (`serde`, `rmp-serde`, `bincode`)
- Unused feature flags
- Redundant functionality

**Your Project**: Each dependency serves a specific, necessary purpose.

## Dependency Management Workflow

### 1. Regular Monitoring
```bash
# Check for updates monthly
cargo outdated

# Review dependency tree quarterly
cargo tree --duplicates
```

### 2. Update Process
```bash
# 1. Check what's outdated
cargo outdated

# 2. Update specific packages (recommended)
cargo update -p package_name

# 3. Verify everything works
cargo test
cargo clippy -- -D warnings
cargo fmt --all -- --check
```

### 3. Adding New Dependencies
**Before adding**:
- Research alternatives
- Check if functionality exists in existing dependencies
- Consider maintenance burden
- Verify license compatibility

**After adding**:
- Update this document
- Run `cargo tree --duplicates` to check for new duplicates
- Test thoroughly

## Best Practices

### ✅ Do
- Keep `src-tauri/Cargo.lock` committed (application backend lockfile)
- Use `cargo outdated` regularly
- Update dependencies incrementally
- Test after updates
- Document dependency decisions

### ❌ Don't
- Edit `Cargo.lock` manually
- Delete `src-tauri/Cargo.lock`
- Update all dependencies at once without testing
- Add dependencies without research
- Ignore security updates

## Troubleshooting

### Common Issues
1. **Build failures after updates**: Use `cargo update -p package_name` to update specific packages
2. **Version conflicts**: Check `cargo tree --duplicates` for conflicting versions
3. **Feature flag issues**: Review feature flags in `Cargo.toml`

### Recovery Commands (Rust backend in `src-tauri/`)
```bash
# Reset backend lockfile to last working state
git checkout HEAD -- src-tauri/Cargo.lock

# Clean and rebuild
cd src-tauri
cargo clean
cargo build

# Check for issues
cargo check
cargo clippy -- -D warnings
```

## Documentation Standards

This document follows Rust documentation conventions:
- **Markdown format**: Standard for Rust documentation
- **Code blocks**: Fenced code blocks for commands and examples
- **Clear structure**: Organized with headings and subheadings
- **Actionable content**: Includes specific commands and workflows

## References

- [Cargo Book](https://doc.rust-lang.org/cargo/)
- [Rust Documentation Conventions](https://doc.rust-lang.org/rustdoc/how-to-write-documentation.html)
- [Semantic Versioning](https://semver.org/)
- [Cargo Tree Documentation](https://doc.rust-lang.org/cargo/commands/cargo-tree.html)

---

**Last Updated**: October 2025  
**Maintained By**: Project Developer  
**Review Schedule**: Quarterly
