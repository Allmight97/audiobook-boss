---
name: repomix-explorer
description: Analyze and explore local or remote repositories with Repomix output artifacts. Use when users ask to inspect codebase structure, discover implementation patterns, estimate scope/tokens, or locate concerns (auth, API, models, errors) quickly without manual file-by-file traversal.
---

# Repomix Explorer

Use this skill to run Repomix, generate a packed repository artifact, and extract high-signal findings fast.

## Execution Contract

1. Resolve target:
- Remote repo (`owner/name` or GitHub URL): use `--remote`.
- Local repo or directory: pass local path or current directory.
2. Choose output path:
- Remote target: always write to `/tmp/...` to avoid polluting the current workspace.
- Local target: default `repomix-output.xml` unless a custom output is useful.
3. Pick scale controls:
- Add `--compress` for large repositories or when token/size is high.
- Add `--include` / `--ignore` when the user asks for scoped analysis.
4. Generate artifact with Repomix.
5. Analyze artifact with `rg` first, then read narrowed sections.
6. Report findings tied to concrete file paths and outcomes.

## Command Patterns

```bash
# Remote
npx repomix@latest --remote owner/repo --output /tmp/repo-analysis.xml

# Local (current directory)
npx repomix@latest

# Local specific directory
npx repomix@latest ./src

# Large repository
npx repomix@latest --remote owner/repo --compress --output /tmp/repo-analysis.xml

# File-type scoping
npx repomix@latest --include "**/*.{ts,tsx,rs,svelte}"
```

## Analysis Workflow

1. Capture command metrics:
- Files processed
- Characters
- Tokens
- Output file path
2. Build structure baseline:
- Find tree/summary section in artifact.
3. Run targeted pattern scans with `rg`:

```bash
rg -i "export\\s+(function|class)|fn\\s|struct\\s|impl\\s" repomix-output.xml
rg -i "auth|login|password|token|jwt" repomix-output.xml
rg -i "router|route|endpoint|api" repomix-output.xml
rg -i "model|schema|database|query|sql" repomix-output.xml
rg -i "error|exception|try|catch|Result<|anyhow|thiserror" repomix-output.xml
```

4. Narrow to relevant segments, then inspect with line-context reads.
5. Summarize:
- Immediate UX/DX impact (what this enables for user decisions now)
- Architectural ripple (module boundaries, coupling hotspots)
- Maintenance signal (risk/churn/testing implications)

## Output Shape

Return:
1. Metrics snapshot (files/tokens/size).
2. Structural map (top-level components and responsibilities).
3. Pattern findings grouped by concern (auth, API, data, errors, etc.).
4. Risks/gaps discovered.
5. High-value next probe (only when it improves decision quality).

## Error Handling

- Repomix command failure:
  - Verify target path/repo syntax, network, and permissions.
  - Retry with explicit output path and reduced scope.
- Artifact too large:
  - Re-run with `--compress` and/or `--include`.
- No matches for requested concern:
  - Broaden patterns and confirm the concern exists in scope.

## Guardrails

- Repomix output format is XML by default and should be treated as required (`.xml` output paths such as `repomix-output.xml` or `/tmp/...xml`).
- Do not use Markdown files as Repomix output artifacts.
- Do not claim findings without evidence from the generated artifact.
- Keep output paths explicit so users can reuse or clean artifacts.
- Clean large `/tmp` artifacts after use if they are no longer needed.
