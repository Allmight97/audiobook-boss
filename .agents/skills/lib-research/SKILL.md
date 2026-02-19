---
name: lib-research
description: Primitive planning tool for implementation specs. Use Exa MCP surgically to remove external-library uncertainty before coding.
---

# Library Research (Planning Primitive)

## Mission
Use this skill as a planning primitive when creating or refining an implementation spec.

Goal: convert uncertain external-library/API behavior into concrete implementation decisions.
Use this same planning primitive during code/PR/local-branch audits when review conclusions depend on external-library behavior.

Tri-order impact lens:
- Immediate UX/DX: faster correct implementation path
- Architectural ripple: safer boundaries/contracts across modules
- Long-term maintenance: fewer regressions from stale or low-authority guidance

## Invoke Criteria (High Value Only)
Invoke before implementation when at least one is true:
- dependency/framework behavior may have changed
- command/API contract is ambiguous
- version-specific behavior can alter design choices
- security/performance guidance is needed for a critical decision
- conflicting sources block confident spec writing
- audit/review findings depend on external API/library behavior that must be validated before sign-off

Do not invoke for pure internal refactors with no external-library uncertainty.

## Agent-Need Router
Pick mode based on need, not tool preference.

1. `spec-snippet`
Need: exact API shape, syntax, minimal usage pattern.
- Primary: `get_code_context_exa`
- Secondary: `crawling_exa` for canonical confirmation

2. `spec-verify`
Need: confirm current behavior, releases, compatibility, deprecations.
- Primary: `web_search_exa`
- Precision upgrade: `web_search_advanced_exa` (domain/date filters)
- Confirm with `crawling_exa` before finalizing claims

3. `spec-synthesis`
Need: many moving parts, conflicting evidence, architectural tradeoff analysis.
- Start `deep_researcher_start`
- Poll `deep_researcher_check` until `completed`
- Re-validate key claims against primary sources

Avoid `company_research_exa` and `people_search_exa` unless explicitly requested.

## Query Construction
- include language + framework/runtime + version
- include exact identifiers (symbols, config keys, errors)
- constrain by domain/date for fast-moving topics
- bias toward primary sources first (official docs, release notes, canonical repos)

## Spec Output Contract (Required)
Return research in spec-ready structure:
1. Decision candidates (2-4 options max)
2. Recommended decision with rationale
3. Constraints/gotchas (version/env/security/perf)
4. Acceptance checks/tests impacted
5. Sources (URLs, primary first)
6. Confidence + unresolved uncertainty

## Escalation Rules
- Start in `spec-snippet` or `spec-verify`.
- Escalate to `spec-synthesis` only when:
  - sources conflict,
  - scope spans multiple libraries/components, or
  - user explicitly asks for deep research.

## Quality Gates
- deduplicate mirrors/reposts/low-authority summaries
- include explicit dates for time-sensitive claims
- separate facts from inference
- fail fast on unresolved uncertainty; surface it in the spec

For tool routing details, use `references/tool-selection.md`.
