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
