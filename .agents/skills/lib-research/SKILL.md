---
name: lib-research
description: Planning primitive for external-library/API uncertainty. Use Exa MCP to verify behavior and turn uncertainty into implementation-ready decisions.
---

# Library Research

Use this skill before implementation or audit sign-off when decisions depend on external library/API behavior.

## When to Use

Trigger when at least one applies:
- API behavior is ambiguous or version-sensitive.
- Security/performance guidance affects architecture decisions.
- Sources conflict or rely on secondary summaries.
- Review findings depend on external-library claims.

Skip for pure internal refactors with no external dependency uncertainty.

## Router

Pick the lightest mode that resolves uncertainty:
1. `spec-snippet` for exact API shape and syntax.
- Primary: `mcp__exa__get_code_context_exa`
- Confirm source page with `mcp__exa__crawling_exa` when needed.
2. `spec-verify` for compatibility, deprecations, or recent changes.
- Primary: `mcp__exa__web_search_exa`
- Use `mcp__exa__web_search_advanced_exa` for domain/date filtering.
3. `spec-synthesis` for cross-library tradeoffs or conflicting evidence.
- `mcp__exa__deep_researcher_start` + `mcp__exa__deep_researcher_check`
- Re-validate key claims against primary docs before final recommendation.

## Query Construction

- Include language, framework/runtime, and version.
- Include exact symbols, config keys, and error identifiers.
- Bias to primary sources first (official docs, release notes, canonical repos).
- Add explicit dates for time-sensitive claims.

## Output Contract

Return results in this shape:
1. Decision options (2-4 max).
2. Recommended decision and rationale.
3. Constraints/gotchas (version/env/security/perf).
4. Acceptance checks/tests impacted.
5. Sources (URLs, primary first).
6. Confidence and unresolved uncertainty.

## Quality Gates

- Separate facts from inference.
- Remove mirror/repost/low-authority sources.
- If uncertainty remains, fail fast and call it out before implementation.

## Alignment

- Use root AGENTS precedence.
- No implicit internal legacy assumptions.
- Fallback behavior requires explicit trigger/evidence/sunset and fallback-policy compliance.

For Exa mode details, see `references/tool-selection.md`.
