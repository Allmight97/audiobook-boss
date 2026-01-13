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
