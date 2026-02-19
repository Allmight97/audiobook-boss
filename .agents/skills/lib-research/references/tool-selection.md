# Library Research Selection Matrix (Spec Planning)

| Agent Need | Mode | Primary | Secondary | Output Focus |
|---|---|---|---|---|
| Need exact API usage to write implementation step | `spec-snippet` | `get_code_context_exa` | `crawling_exa` | minimal snippet + constraints |
| Need current behavior/version verification | `spec-verify` | `web_search_exa` | `crawling_exa` | verified claims + dated sources |
| Need strict domain/date precision | `spec-verify` | `web_search_advanced_exa` | `crawling_exa` | high-precision evidence set |
| Need cross-source tradeoff synthesis | `spec-synthesis` | `deep_researcher_start` | `deep_researcher_check` | recommendation + risks + options |

## Escalation
- default to `spec-snippet` or `spec-verify`
- move to `spec-synthesis` only when conflict/complexity demands it

## Source Priority
1. Official vendor/project docs
2. Maintainer-owned repos and release notes
3. Trusted technical publications
4. Community posts (supporting only)
