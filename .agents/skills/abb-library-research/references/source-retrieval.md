# Exceptional Upstream Source Retrieval

Read when the active question needs tests, examples, internals, or history
omitted from the installed or registry package. Root `AGENTS.md` owns the
policy on retaining upstream snapshots.

Map the resolved package to its upstream revision using registry metadata,
`.cargo_vcs_info.json`, or release evidence. A version string is not proof
that an identically named tag matches the published package. Verify the
packaged source/checksum as applicable and resolve the revision to a commit
SHA before making exact-version claims.

Prefer an API or single-file fetch when it answers the question. For a tree,
tests, or history, retrieve the verified commit into an ephemeral OS-temp
checkout; use shallow, filtered, or sparse retrieval when useful. Read only the
material needed, cite the SHA in the answer, and remove the task-created
checkout after use.

If the published artifact cannot be tied to a revision, report the retrieved
source as unproven for the installed version. Return the implementation
consequence to the ABB owner instead of silently substituting upstream behavior.

A maintained pattern reference earns a place here only after repeated ABB
lookup friction. Before adding one, inspect existing callers and references;
retain the decision-changing idiom and exact source pointer rather than an
upstream tutorial. Refresh or remove it when its evidence no longer matches
the selected package.
