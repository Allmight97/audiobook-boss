---
name: release-changelog
description: Release-note and changelog workflow for Audiobook Boss. Use when drafting/applying changelog entries and executing deterministic release steps.
---

# Release Changelog

Use this skill when the user explicitly asks to cut a release or update `CHANGELOG.md`.

## Inputs

- Target version: `<SEMVER>`
- Release date: `<YYYY-MM-DD>` (default system date if omitted)
- Optional base tag override: `<TAG>`

## Workflow

1. Draft changelog section from merged PR metadata.
2. Show draft and request explicit approval.
3. Apply changelog changes.
4. Run release executor with explicit commit/tag choice.

## Commands

Draft:
```bash
scripts/generate-release-changelog.sh --version <SEMVER> --date <YYYY-MM-DD>
```

Apply:
```bash
scripts/generate-release-changelog.sh --version <SEMVER> --date <YYYY-MM-DD> --apply
```

Release without commit/tag:
```bash
scripts/release.sh --version <SEMVER> --changelog-verified --no-commit-tag
```

Release with commit/tag:
```bash
scripts/release.sh --version <SEMVER> --changelog-verified --commit-tag
```

## Guardrails

- Do not edit version or changelog unless release work is explicitly requested.
- Fail fast if `[Unreleased]` section is missing or target header already exists.
- Never tag automatically without explicit approval.

## Done Criteria

- Changelog entry is approved and structurally valid.
- Release command path matches user-approved commit/tag mode.

## Alignment

- Use root AGENTS precedence.
- No implicit internal legacy assumptions.
- Fallback behavior requires explicit trigger/evidence/sunset and fallback-policy compliance.
