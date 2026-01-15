---
name: lib-research
description: |
  External library documentation and source code research. Use when verifying API contracts,
  understanding library internals, or researching how a dependency implements something.
  Triggers: planning features, verifying API before implementation, checking library behavior,
  debugging unexpected results, refactoring with library dependencies, greenfield design,
  "does [library] support", "how does [library] work".
---

# Library Research

Unified approach to researching external libraries—whether planning greenfield features,
debugging, refactoring, or verifying API contracts.

## Tools

### btca (Primary)

btca delegates research to a smaller, faster model (e.g., Haiku) that searches actual library
source code and returns answers. Think of it as a research assistant that reads the codebase
for you.

```bash
btca ask -r <resource> -q "Your question here"
```

**What it does:**
- Clones library repos locally and indexes them
- Searches source code (not just docs)
- Returns answers with citations from the source

**Multiple resources:**
```bash
btca ask -r svelte -r effect -q "How do I integrate these?"
```

**Avoid in automation:**
```bash
btca        # Launches interactive TUI - blocks execution
btca chat   # Interactive session
```

### Context7 (Companion/Fallback)

Context7 provides curated documentation lookup. Use it alongside btca or when a btca
resource isn't configured.

```
1. mcp__plugin_context7_context7__resolve-library-id  → get library ID
2. mcp__plugin_context7_context7__query-docs          → query documentation
```

## When to Use Which

| Situation | Tool | Why |
|-----------|------|-----|
| How does library implement X? | btca | See actual source |
| Planning feature using library | btca | Understand internals |
| Debugging unexpected behavior | btca | Source reveals edge cases |
| Official API docs/examples | Context7 | Curated documentation |
| btca resource not configured | Context7 | Fallback |
| Cross-reference docs vs source | Both | Catch discrepancies |

## Working with btca Responses

btca's responses come from another model, so treat them as research notes:

**Response has citations (file paths, code)?** Use confidently.

**Response is summary without citations?** Fine for orientation, but consider
querying again if you need specifics.

**Key rule:** Never fabricate file paths or code that weren't in the response.
If btca didn't provide a citation, don't invent one.

## Evidence Labeling

When reporting findings, note the source:

- `source`: From btca with file path/code citation
- `docs`: From Context7 documentation
- `inference`: Your interpretation (note it needs verification)

## Available Resources

See [references/btca-resources.md](references/btca-resources.md) for configured btca resources.

Current resources: tauri, ffmpeg-next, mp4ameta, tokio, vitest, serde, serde-derive, typescript

## References

- btca config: `btca.config.jsonc`
- btca docs: https://btca.dev/getting-started
- btca repo: https://github.com/davis7dotsh/better-context
