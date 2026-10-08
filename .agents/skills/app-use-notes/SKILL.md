---
name: app-use-notes
description: Record what the owner observed while using ABB, and check a product decision against those notes. Use when the owner describes something they saw in the app, or when an issue asks the owner to choose product behavior.
---

# App Use Notes

`docs/agents/observed-use.md` holds what the owner met while using ABB. A
product decision cites it, so an issue argues from a case the owner hit, not
from theory.

## Record a finding

When the owner describes something they saw while using the app, add one entry
at the top of the Findings list:

```md
### 2026-10-08 Short name
- Did: what the owner was doing, with the source kind and the step.
- Saw: what happened, as observed.
- Touches: the Golden Path step or outcome (root `AGENTS.md`), or "none".
- Blocks: what it blocks, or "nothing".
```

Write the observation as stated. A diagnosis or a fix proposal goes in an
issue, not here. Ask for a missing field instead of guessing it.

## Check a product decision

Before drafting or editing an issue that asks the owner to choose what the user
gets (label `decision`, or an "Open forks" section about user-visible
behavior):

1. Search the notes for the behavior.
2. In the issue's next-action line, cite the matching entry or write "no
   observed case".
3. With no observed case, recommend closing or deferring; the owner decides.
   List the question under "Raised without a case" so it is not asked twice.
