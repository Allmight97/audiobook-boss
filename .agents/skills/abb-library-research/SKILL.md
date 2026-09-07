---
name: abb-library-research
description: Resolve version-sensitive Effect, Solid, Tauri, Specta, or tauri-specta behavior when an ABB implementation or contract decision needs external-library evidence.
---

# ABB Library Research

Resolve the active library question and return the answer to its ABB owner.
Repository paths below are relative to the ABB root.

## Evidence

Start with the owning ABB code, tests, and contract, then identify the resolved
package version and source in `bun.lock` or `Cargo.lock`. Manifests state the
intended range; lockfiles select the version.

Use installed declarations/source or exact registry-packaged source for
version-sensitive behavior. Exact-version public documentation may be enough
for a documented API; inspect implementation when the question depends on it.
If installed files are absent or incomplete, retrieve the exact npm tarball or
Cargo crate and verify its version and registry checksum as applicable.

Load only the route card for the library involved:

| Question | Reference |
| --- | --- |
| Effect workflows and APIs | [effect.md](references/effect.md) |
| Solid rendering, reactivity, or component tests | [solid.md](references/solid.md) |
| Tauri runtime, commands, capabilities, or bundling | [tauri.md](references/tauri.md) |
| Installed Tauri plugins | [tauri-plugins.md](references/tauri-plugins.md) |
| Rust type export and TypeScript generation | [specta.md](references/specta.md) |
| Tauri command/event codegen integration | [tauri-specta.md](references/tauri-specta.md) |

When package evidence leaves a material gap, use available documentation or
search tools to find primary docs, source, issues, or history. Context7 can help
with a known API; Exa or web search can discover sources. Resolve Context7 IDs
live if using it; route-card IDs are hints. Neither tool is required. Reconcile
current docs and search results to the resolved package before treating them
as version proof.

Read [source-retrieval.md](references/source-retrieval.md) when omitted tests,
examples, codegen internals, or history require upstream retrieval. Root
`AGENTS.md` owns the policy on upstream source snapshots; external research
does not transfer product ownership out of ABB's local boundaries.

## Finish

Report the behavior, ABB owner, implementation-changing constraints, and
version/source evidence. Record the commit SHA when upstream source was used.
If evidence is unavailable or conflicts, label the answer current-only,
version-mismatched, or unproven and state the remaining decision.

Stop once the active question is answered with adequate version evidence, or
the missing evidence and its implementation consequence are explicit.
