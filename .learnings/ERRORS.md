# Errors


## [ERR-20260111-001] gh-issue-view

**Logged**: 2026-01-11T22:25:20Z
**Priority**: high
**Status**: pending
**Area**: infra

### Summary
Failed to resolve GitHub repo when running `gh issue view` with an incorrect owner.

### Error
```
GraphQL: Could not resolve to a Repository with the name 'jstar/audiobook-boss'. (repository)
```

### Context
- Command/operation attempted: `gh issue view 143 --repo jstar/audiobook-boss --json ...`
- Input or parameters used: assumed owner `jstar` instead of checking git remote
- Environment details: local repo uses `origin https://github.com/Allmight97/audiobook-boss.git`

### Suggested Fix
Check `git remote -v` (or `gh repo view`) before calling `gh issue view` to ensure correct owner/repo.

### Metadata
- Reproducible: yes
- Related Files: (none)
- See Also: (none)

---

## [ERR-20260111-002] python-edit-script

**Logged**: 2026-01-11T23:04:19Z
**Priority**: medium
**Status**: pending
**Area**: infra

### Summary
Python script failed while editing `src/ui/fileList/actions.ts` due to an unterminated string literal.

### Error
```
SyntaxError: unterminated string literal (detected at line 5)
```

### Context
- Command/operation attempted: `python3 - <<'PY' ...`
- Input or parameters used: multi-line string literal without triple quotes in `text.replace(...)`

### Suggested Fix
Use triple-quoted strings for multi-line replacements in Python edit scripts.

### Metadata
- Reproducible: yes
- Related Files: src/ui/fileList/actions.ts
- See Also: (none)

---
## [ERR-20260112-001] bun_add_opencode_antigravity

**Logged**: 2026-01-12T21:39:35Z
**Priority**: medium
**Status**: pending
**Area**: config

### Summary
bun add for opencode-antigravity-auth was rejected by user approval policy

### Error
```
exec_command failed: CreateProcess { message: "Rejected(\"rejected by user\")" }
```

### Context
- Command/operation attempted: bun add opencode-antigravity-auth@beta
- Input or parameters used: ran in ~/.config/opencode with restricted network
- Environment details if relevant: approval policy on-request; command required escalation

### Suggested Fix
Wait for user to enable permissions or set approval policy to never / allow escalation, then retry install

### Metadata
- Reproducible: yes
- Related Files: /Users/jstar/.config/opencode/package.json
- See Also: 

---
## [ERR-20260112-002] opencode-run-config-frontmatter

**Logged**: 2026-01-12T21:57:06Z
**Priority**: high
**Status**: pending
**Area**: config

### Summary
`opencode run` failed due to invalid YAML frontmatter in a local skill file.

### Error
```
ConfigFrontmatterError: Failed to parse YAML frontmatter: incomplete explicit mapping pair; a key node is missed; or followed by a non-tabulated empty line at line 3, column 200
```

### Context
- Command: `opencode run "Hello" --model=google/antigravity-claude-sonnet-4-5-thinking --variant=max`
- File: `.claude/skills/lib-research/SKILL.md`
- Cause: colon+space in unquoted YAML scalar (`Triggers: ...`) broke frontmatter parsing.

### Suggested Fix
Convert `description` to a block scalar or quote the string so `:` is treated as text.

### Metadata
- Reproducible: yes
- Related Files: .claude/skills/lib-research/SKILL.md

---
## [ERR-20260113-002] btca

**Logged**: 2026-01-13T09:56:30-08:00
**Priority**: medium
**Status**: pending
**Area**: infra

### Summary
btca chat launches OpenCode TUI and does not return a plain-text answer in this CLI session

### Error
```
TUI launched (OpenCode) with ANSI screen control codes; no answer returned to stdout. Required manual kill (SIGKILL).
```

### Context
- Command/operation attempted: btca chat -r mp4ameta
- Input or parameters used: “Where is FreeformIdent defined? Provide file path and snippet.”
- Environment details if relevant: non-interactive CLI session; TUI not suitable for capture

### Suggested Fix
Prefer btca ask for scripted runs, or run btca chat in an interactive terminal and copy the response back.

### Metadata
- Reproducible: yes
- Related Files: btca.config.jsonc
- See Also: ERR-20260113-001

---
## [ERR-20260113-003] btca

**Logged**: 2026-01-13T10:05:30-08:00
**Priority**: high
**Status**: pending
**Area**: infra

### Summary
btca ask consistently times out after 10 seconds across resources (mp4ameta, tokio, tauri)

### Error
```
[Bun.serve]: request timed out after 10 seconds. Pass `idleTimeout` to configure.
Error: The socket connection was closed unexpectedly. For more information, pass `verbose: true` in the second argument to fetch()
```

### Context
- Command/operation attempted: btca ask -r mp4ameta / tokio / tauri
- Input or parameters used: FreeformIdent / spawn_blocking / event emission prompts
- Environment details if relevant: both anthropic/claude-haiku-4-5-20251001 and opencode/glm-4.7-free

### Suggested Fix
Determine how to raise the Bun.serve idleTimeout for btca server or run against a long-lived btca server via --server.

### Metadata
- Reproducible: yes
- Related Files: btca.config.jsonc
- See Also: ERR-20260113-001, ERR-20260113-002

---
## [ERR-20260113-004] rg

**Logged**: 2026-01-13T11:12:00-08:00
**Priority**: low
**Status**: pending
**Area**: infra

### Summary
rg search over home directory timed out while looking for opencode config files

### Error
```
rg: Operation timed out (os error 60)
```

### Context
- Command/operation attempted: rg --files -g 'opencode*.json*' -g '.opencode*' ~
- Input or parameters used: search for opencode config
- Environment details if relevant: large home directory scan

### Suggested Fix
Limit search to likely config locations (e.g., ~/.config, ~/.opencode) or use find with depth.

### Metadata
- Reproducible: unknown
- Related Files: 
- See Also: 

---
## [ERR-20260115-001] gh-issue-comment

**Logged**: 2026-01-15T20:18:00Z
**Priority**: high
**Status**: pending
**Area**: docs

### Summary
Backticks in `gh issue comment -b "..."` were interpreted by zsh, corrupting the comment body.

### Error
```
zsh:1: permission denied: src/ui/statusPanel/logic.ts
zsh:1: command not found: state.ts
zsh:1: command not found: formatting.ts
zsh:1: command not found: render.ts
```

### Context
- Command: gh issue comment 78 -b "Follow-up ... `src/ui/statusPanel/logic.ts` ... `python3 scripts/analyze_code_lines.py` ..."
- Shell: zsh

### Suggested Fix
Use a heredoc or a small Python wrapper to pass the comment body, or avoid backticks in shell-quoted strings.

### Metadata
- Reproducible: yes
- Related Files: 
- See Also: 

---

## [ERR-20260118-001] scripts/quick-checks.sh

**Logged**: 2026-01-18T17:25:53.912961+00:00
**Priority**: high
**Status**: resolved
**Area**: tests

### Summary
Quick checks failed after removing metadata_to_ffmpeg_dict wrapper; test referenced wrong module path

### Error
```
error[E0433]: failed to resolve: could not find `ffmpeg_dict` in `super`
  --> src-tauri/src/metadata/ffmpeg_bridge.rs:33:34
   |
33 |         let dict_result = super::ffmpeg_dict::metadata_to_ffmpeg_dict(&metadata);
   |                                  ^^^^^^^^^^^ could not find `ffmpeg_dict` in `super`
   |
help: consider importing this module
   |
17 +     use crate::metadata::ffmpeg_dict;
   |
help: if you import `ffmpeg_dict`, refer to it directly
   |
33 -         let dict_result = super::ffmpeg_dict::metadata_to_ffmpeg_dict(&metadata);
33 +         let dict_result = ffmpeg_dict::metadata_to_ffmpeg_dict(&metadata);
   |

error: unused import: `super::*`
  --> src-tauri/src/metadata/ffmpeg_bridge.rs:17:9
   |
17 |     use super::*;
   |         ^^^^^^^^
   |
   = note: `-D unused-imports` implied by `-D warnings`

error: could not compile `audiobook-boss` (lib test) due to 2 previous errors
```

### Context
- Command: `scripts/quick-checks.sh`
- Occurred after removing wrapper function in `src-tauri/src/metadata/ffmpeg_bridge.rs`

### Suggested Fix
Update test module to import `crate::metadata::ffmpeg_dict` or use `crate::metadata::ffmpeg_dict::metadata_to_ffmpeg_dict` directly; remove unused `super::*` import.

### Metadata
- Reproducible: yes
- Related Files: src-tauri/src/metadata/ffmpeg_bridge.rs
- See Also: None


### Resolution
- **Resolved**: 2026-01-18T17:26:29.994482+00:00
- **Commit/PR**: 53ca303 (follow-up fix pending commit)
- **Notes**: Updated test to call metadata_to_ffmpeg_dict via module path.

---

## [ERR-20260118-002] scripts/quick-checks.sh

**Logged**: 2026-01-18T17:26:55.770458+00:00
**Priority**: high
**Status**: resolved
**Area**: tests

### Summary
Quick checks failed due to unused import left in ffmpeg_bridge test module

### Error
```
error: unused import: `super::*`
  --> src-tauri/src/metadata/ffmpeg_bridge.rs:17:9
   |
17 |     use super::*;
   |         ^^^^^^^^
   |
   = note: `-D unused-imports` implied by `-D warnings`

error: could not compile `audiobook-boss` (lib test) due to 1 previous error
```

### Context
- Command: `scripts/quick-checks.sh`
- File: `src-tauri/src/metadata/ffmpeg_bridge.rs`

### Suggested Fix
Remove the unused `use super::*;` import in the test module.

### Metadata
- Reproducible: yes
- Related Files: src-tauri/src/metadata/ffmpeg_bridge.rs
- See Also: ERR-20260118-001


### Resolution
- **Resolved**: 2026-01-18T17:27:02.487893+00:00
- **Commit/PR**: pending
- **Notes**: Removed unused import from ffmpeg_bridge test module.

---

## [ERR-20260118-003] scripts/quick-checks.sh

**Logged**: 2026-01-18T17:27:42.877682+00:00
**Priority**: high
**Status**: resolved
**Area**: tests

### Summary
TypeScript check failed due to unused imports after removing window.testCommands

### Error
```
src/main.ts(6,3): error TS6133: 'displayFileList' is declared but its value is never read.
src/main.ts(9,3): error TS6133: 'clearAllFiles' is declared but its value is never read.
src/main.ts(10,3): error TS6133: 'toggleFileSort' is declared but its value is never read.
src/main.ts(11,3): error TS6133: 'moveFileUp' is declared but its value is never read.
src/main.ts(12,3): error TS6133: 'moveFileDown' is declared but its value is never read.
src/main.ts(17,3): error TS6133: 'getCurrentOutputConfig' is declared but its value is never read.
src/main.ts(18,3): error TS6133: 'onFileListChange' is declared but its value is never read.
src/main.ts(19,3): error TS6133: 'onMetadataChange' is declared but its value is never read.
src/main.ts(25,3): error TS6133: 'getCurrentCoverArt' is declared but its value is never read.
src/main.ts(26,3): error TS6133: 'setCoverArt' is declared but its value is never read.
src/main.ts(27,3): error TS6133: 'clearCoverArt' is declared but its value is never read.
src/main.ts(31,26): error TS6133: 'updateTagPreview' is declared but its value is never read.
```

### Context
- Command: `scripts/quick-checks.sh`
- File: `src/main.ts`

### Suggested Fix
Remove unused imports from `src/main.ts` after deleting the console test harness.

### Metadata
- Reproducible: yes
- Related Files: src/main.ts
- See Also: ERR-20260118-002


### Resolution
- **Resolved**: 2026-01-18T17:27:49.181922+00:00
- **Commit/PR**: pending
- **Notes**: Removed unused imports from src/main.ts.

---
