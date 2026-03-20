---
name: task-tracker
description: Task tracking and done-loop verification for Audiobook Boss using pebbles (pb). Use when starting work on a planned item, checking what is ready to work on, marking a task done, or verifying that completed work matches the plan. Always load this skill at the start of a coding session.
---

# Task Tracker

This project tracks work in **pebbles** (`pb`), an append-only JSONL issue tracker.
Data lives in `.pebbles/events.jsonl` — committed alongside code.

`pb` is installed at `~/.local/bin/pb`. Always invoke it as:
```bash
export PATH="/Users/jstar/.local/bin:$PATH" && cd /Users/jstar/Projects/audiobook-boss && pb <command>
```

## Session Start Protocol

Run this at the start of every coding session:

```bash
export PATH="/Users/jstar/.local/bin:$PATH" && cd /Users/jstar/Projects/audiobook-boss
pb ready          # unblocked work
pb list           # full open/in-progress list
```

Read the output. If an item is `in_progress`, that is the active task — resume it.
If nothing is in-progress, pick the highest-priority ready item.

## Starting a Task

```bash
pb update <issue-id> --status in_progress
```

Then read the issue description in full:

```bash
pb show <issue-id>
```

The description contains the exact files to change, the action to take, and the
verification command. Do exactly what it says — no more, no less.

## Done-Loop: Verification Before Closing

**Do not close an issue until verification passes.**

Every task description ends with a `Verify:` line. Run it. If it is
`scripts/checks.sh standard`, that must exit 0.

After verification passes, do a self-check:
1. Read `pb show <issue-id>` again.
2. Confirm each file mentioned in the description was actually changed.
3. Confirm no files were changed that the description did not mention.

If there is a mismatch — you changed something the plan did not cover, or missed
something the plan required — do not close the issue. Either fix the mismatch or
create a new issue for the undocumented change before closing this one.

## Closing a Task

```bash
pb close <issue-id>
```

Then commit, referencing the issue ID:

```bash
git add -A
git commit -m "<type>: <short description>

Closes abb-<id>.
<summary of what changed>"
```

## Discovering New Work During Implementation

If you find something unexpected while working:

```bash
pb create --title="<what you found>" --type=bug|task --priority=P0-P4 \
  --description="<context, file, what needs to change>"
```

Do not let the discovery derail the current task unless it is a blocker (P0).

## Quick Reference

```bash
pb ready                          # unblocked issues (start here)
pb list                           # all open + in-progress
pb list --status closed           # recently completed
pb show <id>                      # full issue detail
pb update <id> --status in_progress
pb close <id>
pb create --title="..." --type=task --priority=P2 --description="..."
pb dep add <id-a> <id-b>          # a is blocked by b
```

## Current Plan Source

Use `pb ready`, `pb list`, and `pb show <id>` as the source of truth for active work.
Treat `docs/specs/*` as supporting plan material only when a live PB issue points to it.
Do not assume hardcoded issue IDs or historical plan docs are still current.
