---
name: lib-research
description: Planning primitive for external-library/API uncertainty. Use Ref first for canonical docs, Context7 second for curated library docs, and standard web for current verification.
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
- Primary: `mcp__ref__ref_search_documentation` to find the canonical page/section.
- Then: `mcp__ref__ref_read_url` for the exact section when Ref has the right target.
- Then: Context7 (`resolve-library-id` -> `query-docs`) when the library is indexed and you want curated library docs or examples after locating the canonical surface.
2. `spec-verify` for compatibility, deprecations, or recent changes.
- Primary: standard web search/open (`web.search_query` + `web.open`) for current primary sources.
- Use Ref again if the question is still documentation-shaped and needs better anchors.
3. `spec-synthesis` for cross-library tradeoffs or conflicting evidence.
- Prefer local synthesis from Ref + Context7 + standard web evidence first.
- Re-validate key claims against primary docs before final recommendation.

## Query Construction

- Include language, framework/runtime, and version.
- Include exact symbols, config keys, and error identifiers.
- Bias to primary sources first (official docs, release notes, canonical repos).
- Add explicit dates for time-sensitive claims.
- When using Ref, write docs-shaped queries around the exact symbol or section name.
- When using Context7, prefer the official docs-site library ID over repo mirrors when both are available.
- When using standard web, prefer official-domain intent in the query and open the primary source directly.

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
- Prefer canonical docs anchors over broad result lists when both answer the question.
- In most cases, Ref plus Context7 plus standard web should resolve the question.
- If uncertainty remains, fail fast and call it out before implementation.

## Alignment

- Use root AGENTS precedence.
- No implicit internal legacy assumptions.
- Fallback behavior requires explicit trigger/evidence/sunset and fallback-policy compliance.

For routing details, see `references/tool-selection.md`.
