# Preview Audio

## Public API Strip

- `ProcessSplitButton`

## Ownership

- Own the Process split control: the primary main action starts a full run and
  the caret menu starts retained foreground preview processing
  (15/30/45/60s) — both through the Status Panel public strip. This owner does
  not read work operation snapshots.
