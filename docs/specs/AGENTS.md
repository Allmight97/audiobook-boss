# Specs Directory Directives

## Scope

- Applies to active planning, roadmap, and implementation specs under
  `docs/specs/`.
- Specs are temporary work packets for substantial multi-session or multi-agent
  efforts, not permanent repo canon.

## Folder Contract

- Create or update specs only through explicit repo-owner alignment or the
  repo's active planning workflow.
- Keep one active Markdown spec per substantial effort or roadmap.
- Keep the spec self-contained enough that a fresh agent can validate and
  continue from the current repo state plus the spec.
- Keep the spec current while work proceeds: update progress, discoveries,
  decisions, proof status, and remaining work at meaningful stopping points.
- Move ephemeral chat logs, after reports, generated companions, and
  presentation artifacts to `/Users/jstar/Documents/Codex/artifacts/audiobook-boss`.

## Hard Invariants

- Do not use `docs/specs/` as a permanent feature catalog, session transcript
  archive, issue tracker, or report dump.
- Do not let a spec silently redefine canon surfaces such as `AGENTS.md`,
  `docs/api-map.md`, `docs/system-map.md`, `docs/ubiquitous-language.md`, or
  nested `AGENTS.md` files.
- When the effort is implemented, rejected, or superseded, delete the spec or
  distill only enduring truths into the owning canon surfaces.

## Done Criteria

- The spec guides execution while active without becoming its own process
  burden.
- Completion cleanup has an explicit destination: delete, or distill to canon
  docs/issues/changelog/release notes as appropriate.
- Generated companions remain external unless the repo owner explicitly asks for
  a repo-local artifact.
