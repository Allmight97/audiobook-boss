# Exceptional Upstream Source Retrieval

Use this reference when the active library-research question needs upstream
tests, omitted examples, codegen internals, runtime implementation, or history
that installed and registry packages do not contain.

## Control-Plane Rule

`abb-library-research` owns the reference-library workflow:

- resolved versions come from `bun.lock` and `Cargo.lock`
- route cards live in this skill's `references/<library>.md`
- task-specific pattern files, if needed, live as
  `references/pattern-<library>-<topic>.md`
- root `AGENTS.md` keeps the no-snapshot invariant; this file owns the
  retrieval procedure

Do not create a second routing system under `repos/`, `docs/reference/`, or a
repo-local ticket ledger. Do not write upstream checkouts into the ABB tree.

## When To Retrieve Upstream Source

Stay on installed or registry-packaged source plus Context7 and exact public
docs unless the question cannot be answered from those.

Never infer that a package version equals an upstream tag. Resolve the
repository and a candidate tag or revision, verify that it corresponds to the
published package, resolve it to a commit SHA, and record that SHA in the
research answer.

Prefer the GitHub API or a single-file fetch when targeted files are enough.
Create an ephemeral OS-temp clone only when a tree, tests, or history is
required. Prefer shallow, filtered, or sparse retrieval. Remove the temporary
checkout after the task.

## Retrieval Shape

1. Read the resolved version from `bun.lock` or `Cargo.lock`.
2. Identify the upstream repository from the route card or package metadata.
3. Map version → candidate tag/revision using crates.io, npm, or GitHub
   releases. Confirm the published artifact (checksum, packument, or crate
   `.crate` source) matches that revision.
4. Fetch the verified commit. Record the SHA in the answer.
5. Read only the files needed for the question.
6. Delete the temporary checkout before finishing.

## Pattern Files

Do not create pattern files speculatively. Create one only after a real ABB task
shows repeated lookup friction for the same external-library idiom.

Pattern files must:

- live one level deep under `references/`
- be named `pattern-<library>-<topic>.md`
- cite concrete upstream source, test, or docs paths
- describe practical ABB usage, not broad upstream documentation
- include avoid-notes when they prevent likely misuse
- be refreshed or deleted when cited source paths stop matching lockfile truth
