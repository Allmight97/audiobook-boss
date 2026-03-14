# Library Research Selection Matrix (Spec Planning)

| Agent Need | Mode | Primary | Secondary | Output Focus |
|---|---|---|---|---|
| Need exact API section/page | `spec-snippet` | `ref_search_documentation` | `ref_read_url` | canonical anchor + exact doc text |
| Need curated library docs after locating the library | `spec-snippet` | `context7 resolve-library-id` + `query-docs` | `ref_read_url` | library-focused examples + doc summary |
| Need current behavior/version verification | `spec-verify` | `web.search_query` + `web.open` | `ref_search_documentation` | verified claims + dated primary sources |
| Need cross-source tradeoff synthesis beyond manual combination of Ref, Context7, and web sources | `spec-synthesis` | manual synthesis | targeted follow-up reads via Ref/web | recommendation + risks + options |

## Escalation
- default to `spec-snippet` or `spec-verify`
- move to `spec-synthesis` only when conflict/complexity demands it
- Ref plus Context7 should handle most library/API questions
- use standard web search/open when the question is current, broader, or outside library docs

## Source Priority
1. Official vendor/project docs
2. Maintainer-owned repos and release notes
3. Trusted technical publications
4. Community posts (supporting only)
