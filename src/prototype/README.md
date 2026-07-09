# UI redesign prototype shell

Browser-only living prototype for the **new** UI direction (#412). This is the
reaction surface for owner review — not production code, not the current app.

## Launch

```bash
bun run dev
```

Open **http://localhost:1420/prototype.html**

## What this is vs other dev surfaces

| Surface | URL | Purpose |
| --- | --- | --- |
| Current app | `/` | Production islands (unchanged until Slice 3) |
| Design lab | `/lab.html` | Token + primitive catalog (ingredients) |
| **Direction shell** | `/prototype.html` | **Full v3 window mock** on branch `ui/redesign-prototype` |
| Static mock | `docs/design/ui-directions-v3.html` | Lineage reference; open in any browser |

## What is real vs mocked

| Real | Mocked |
| --- | --- |
| Svelte shell, `styles.css` tokens, `app-cover-thumb`, `app-progress-track` | All file data, encode/processing, metadata save |
| Density switch (`[data-density='compact']`) | Tauri IPC, backend |
| Open-fork toggles (rail/popover, selection, ops pin) | Import, Process, lookup actions |

## Open forks (live toggles above the window)

1. **Edit surface** — persistent rail vs anchored popover
2. **Selection demo** — single file vs 3-file batch metadata
3. **Ops panel** — collapsed vs pinned open

Settle these before Slice 3 island rebuild.

## Porting to production

Cherry-pick layout/token decisions into `main` as focused slices after the
direction locks. Do not merge this shell into the app build without an explicit
decision.
