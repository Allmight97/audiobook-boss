# PreToolUse Hook Command Path Alignment

## Current Decision

Keep only the Codex `PreToolUse` Bash guard in repo hook config. Remove advisory prompt/session hooks and Stop-time verification reminders from the lifecycle hook surface.

The retained hook is intentionally narrow: it blocks destructive shell commands such as broad `git reset --hard`, `git checkout -- <path>`, broad `git restore`, `git clean -fd`, and broad `rm -rf` patterns. It does not replace the repo verification commands in `README.md` or `AGENTS.md`.

## What "Robust Command Path" Means

A robust hook command path must fail quietly when Codex is no longer executing inside a valid ABB git worktree. The previous lifecycle hooks resolved scripts with:

```sh
python3 "$(git rev-parse --show-toplevel)/.codex/hooks/<script>.py"
```

That is brittle when a Codex thread remains anchored to a deleted or invalid worktree. In that failure mode, `git rev-parse --show-toplevel` can fail and the hook command may collapse into a bad absolute path such as `/.codex/hooks/...`, producing noisy errors unrelated to the repo's real health.

The retained hook now resolves the repo root with stderr suppressed, exits successfully if no git worktree is available, checks that the target script exists, and only then runs Python:

```sh
repo_root="$(git rev-parse --show-toplevel 2>/dev/null)" || exit 0
hook="$repo_root/.codex/hooks/pre_tool_use_guard.py"
[ -f "$hook" ] || exit 0
python3 "$hook"
```

## Re-enable Criteria

Before re-enabling this hook in local Codex state, align on these points:

1. The hook should be allowed to do only command-safety checks.
2. The hook should fail quiet when the thread is outside a valid ABB worktree.
3. The hook should block only clearly destructive commands, not ordinary edits, reads, checks, or git inspection.
4. Verification and policy checks should remain explicit repo commands, not lifecycle Stop hooks.

## Acceptable Next Actions

- Keep all ABB hooks disabled locally and treat this repo config as dormant safety scaffolding.
- Re-enable only the `PreToolUse` hook after confirming the local hook trust state points at `.codex/hooks.json:pre_tool_use:0:0`.
- If it becomes noisy again, remove repo hook config entirely and keep destructive-command guidance only in `AGENTS.md`.
