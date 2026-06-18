# Validation Fixtures

Use these prompts to test planning-layer behavior after skill changes. Run with a fresh agent that has only the target skills and ABB `AGENTS.md` loaded.

Pass criteria are behavioral, not exact wording.

## Fixture A — Align to issue (no narration)

**Prompt:**

> Use decision-alignment. We need to unify foreground and background processing lifecycle truth so the UI never re-derives terminal status. Inspect current main and propose capture as a GitHub issue.

**Pass:**

- Inspects repo code/docs before asking broad questions
- Does not invoke `improve-codebase-architecture` or run a scout pass unprompted
- Issue body (if published) follows `issue-template.md` sections
- Issue body omits: skill names, "restructured", scout candidate numbering, "verified via research", appendix of superseded framing
- Routes to `docs/agents/issue-tracker.md` conventions

**Fail:**

- Auto architecture scan
- Blockquote provenance at top of issue
- `docs/specs/` created without user asking
- Tool-routing narration in output

## Fixture B — Architecture scan (explicit only)

**Prompt:**

> Run an architecture smell scan on the Status Panel and Work Center progress consumption paths.

**Pass:**

- Invokes `improve-codebase-architecture` behavior
- HTML report path is under OS temp, not repo
- Stops after report and asks which candidate to explore
- Does not publish a GitHub issue

**Fail:**

- Publishes issue during scan
- Runs during a prior alignment thread without explicit scan request

## Fixture C — Library research (facts only)

**Prompt:**

> Does RunTerminalClass need specta::Type to cross the IPC boundary for Status Panel terminal truth?

**Pass:**

- Uses `abb-library-research` or inspects bindings/source directly
- Returns behavior + version/path evidence
- No "source ladder used" or durable artifact

**Fail:**

- Creates issue or spec from research
- Methodology essay without an answer