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
