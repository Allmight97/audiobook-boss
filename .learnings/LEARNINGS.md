# Learnings

## [LRN-20260110-001] best_practice

**Logged**: 2026-01-10T00:00:00Z
**Priority**: low
**Status**: pending
**Area**: docs

### Summary
Adopt ADRs in `docs/decisions/` to record key product decisions.

### Details
Decisions about output naming defaults and related trade-offs were difficult to reconstruct later. A lightweight ADR template plus per-decision files make it easier to review intent and rationale in future work.

### Suggested Action
Create an ADR template in `docs/decisions/` and add ADRs for notable choices.

### Metadata
- Source: conversation
- Related Files: docs/decisions/000-template.md, docs/decisions/001-abs-output-naming-defaults.md
- Tags: decisions, adr, documentation

---

## [LRN-20260111-001] best_practice

**Logged**: 2026-01-11T00:00:00Z
**Priority**: medium
**Status**: pending
**Area**: docs

### Summary
Treat Audiobookshelf docs as the authoritative source for naming rules.

### Details
The ABS scanner has explicit rules for how publish year and series sequence must appear in folder names. ADRs, UI copy, and output logic need to stay aligned to those rules to avoid drift and confusing output previews.

### Suggested Action
When ABS docs drive a naming decision, update both ADRs and UI hints alongside code changes.

### Metadata
- Source: https://www.audiobookshelf.org/docs#book-title-folder-naming
- Related Files: docs/decisions/001-abs-output-naming-defaults.md, src-tauri/src/audio/output_path.rs, src/ui/outputPanel/pathBuilder.ts, index.html
- Tags: audiobookshelf, naming, documentation

---

## [LRN-20260111-002] best_practice

**Logged**: 2026-01-11T00:00:00Z
**Priority**: medium
**Status**: pending
**Area**: frontend

### Summary
Add explicit UI spacing guardrails with a safe escape hatch to reduce agent-driven layout drift.

### Details
Agents tend to introduce arbitrary spacing values or footer hacks when adjusting layout. Documenting approved spacing tokens and layout patterns in AGENTS, plus a rule to add new tokens via `src/styles.css`, keeps the UI consistent without blocking necessary changes.

### Suggested Action
When adjusting UI spacing or layout, use the documented tokens and patterns. If a new spacing size is required, add it to the AGENTS table and `src/styles.css`.

### Metadata
- Source: conversation
- Related Files: AGENTS.md, src/styles.css
- Tags: ui, spacing, guardrails, agents

---
## [LRN-20260112-001] correction

**Logged**: 2026-01-12T21:40:55Z
**Priority**: medium
**Status**: pending
**Area**: config

### Summary
Use shell/CLI editing tools for user-requested config changes; avoid Python unless user asks

### Details
Used a Python one-off to edit ~/.config/opencode/opencode.json; user explicitly asked to use tools to edit the file instead.

### Suggested Action
Prefer CLI-native editing (e.g., jq or cat + here-doc) for config changes unless user approves Python.

### Metadata
- Source: user_feedback
- Related Files: /Users/jstar/.config/opencode/opencode.json
- Tags: tooling, user_preference
- See Also: 

---
## [LRN-20260114-001] best_practice

**Logged**: 2026-01-14T23:30:00Z
**Priority**: high
**Status**: promoted
**Area**: docs
**Promoted**: .claude/skills/lib-research/SKILL.md

### Summary
btca is a research delegation tool - don't impose rigid query templates that flow through to the smaller model

### Details
btca delegates research to a smaller, faster model (e.g., Haiku) that searches library source code. When writing skill instructions for btca usage:

1. **Don't mandate query formats** - rigid templates like "YOU MUST INCLUDE FILE PATH" can confuse the smaller model or cause it to hallucinate to comply
2. **btca is designed to return citations** - it naturally provides "receipts" (file paths, code snippets) without demanding them
3. **Validation happens on output, not input** - the primary agent validates btca's response, not the query format
4. **Anti-hallucination is about output** - "don't fabricate what btca didn't return" not "demand specific output format"

The skill instructions guide the *primary agent* on how to work with btca's output, not instructions that flow through to btca.

### Suggested Action
When writing skills that use btca, frame guidance as "how to evaluate responses" not "how to structure queries."

### Metadata
- Source: conversation
- Related Files: .claude/skills/lib-research/SKILL.md
- Tags: btca, skills, lib-research, agent-delegation

---
## [LRN-20260114-002] best_practice

**Logged**: 2026-01-14T23:30:00Z
**Priority**: medium
**Status**: promoted
**Area**: docs
**Promoted**: .claude/skills/lib-research/SKILL.md, domain skills

### Summary
Domain skills should defer verification mechanics to lib-research, not duplicate tool instructions

### Details
Domain skills (mp4ameta-patterns, ffmpeg-next-patterns, etc.) capture project-specific patterns that have already been verified. They should:

1. **Focus on patterns** - the known, verified usage for this project
2. **Defer verification to lib-research** - "if you need to verify, go deeper, or something seems stale, use lib-research"
3. **Avoid duplicating tool names** - tool names change; let lib-research own that

This creates a clean separation:
- lib-research: "how to look things up"
- domain skills: "what we already know"

Cross-checking between domain skills and lib-research can improve domain skills when stale info is found.

### Suggested Action
Use standard "Tool Cross-Check" section in domain skills that points to lib-research.

### Metadata
- Source: conversation
- Related Files: .claude/skills/lib-research/SKILL.md, .claude/skills/mp4ameta-patterns/SKILL.md, .claude/skills/ffmpeg-next-patterns/SKILL.md, .claude/skills/audiobook-metadata/SKILL.md, .claude/skills/tauri-command-conventions/SKILL.md
- Tags: skills, lib-research, domain-skills, separation-of-concerns

---
## [LRN-20260114-003] correction

**Logged**: 2026-01-14T23:30:00Z
**Priority**: medium
**Status**: pending
**Area**: config

### Summary
btca `list` command launches interactive TUI - use `btca ask -r <resource> -q "..."` for non-interactive queries

### Details
When testing btca, running `btca list` or `btca` without arguments launches an interactive TUI that blocks execution. For automation/agent use:

- `btca ask -r <resource> -q "question"` - non-interactive single query
- `btca chat -r <resource>` - interactive (avoid in automation)
- `btca` - interactive TUI (avoid in automation)

Check available resources via config file (`btca.config.jsonc`) or references doc, not by running `btca list`.

### Suggested Action
In automation, always use `btca ask -r <resource> -q "..."` pattern.

### Metadata
- Source: error
- Related Files: btca.config.jsonc, .claude/skills/lib-research/SKILL.md
- Tags: btca, automation, cli

---
## [LRN-20260112-002] correction

**Logged**: 2026-01-12T21:43:31Z
**Priority**: medium
**Status**: pending
**Area**: config

### Summary
When asked to remove a provider’s models, remove the entire provider entry if requested to delete the provider

### Details
I emptied .provider.quotio.models instead of removing the .provider.quotio entry entirely; user clarified they wanted the provider removed.

### Suggested Action
When a user asks to remove a provider’s models, confirm whether to delete the provider; if they emphasize “entire entry,” remove the provider object.

### Metadata
- Source: user_feedback
- Related Files: /Users/jstar/.config/opencode/opencode.json
- Tags: config, provider, user_preference
- See Also: 

---

## [LRN-20260115-001] correction

**Logged**: 2026-01-15T00:00:00Z
**Priority**: medium
**Status**: pending
**Area**: tooling

### Summary
Use a heredoc or `--body-file` when creating GitHub issues with `gh` to avoid shell interpretation of backticks or paths.

### Details
Running `gh issue create --body "...` with unescaped backticks and paths can cause the shell to interpret them as commands, leading to errors and a malformed issue body. A heredoc (or `--body-file`) avoids shell expansion and preserves Markdown formatting.

### Suggested Action
Prefer `gh issue create --body "$(cat <<'EOF' ... EOF)"` or `gh issue create --body-file <file>` for multi-line issue bodies.

### Metadata
- Source: command_failure
- Related Files: scripts/ (usage pattern), .learnings/LEARNINGS.md
- Tags: gh, tooling, shell, escaping

---
