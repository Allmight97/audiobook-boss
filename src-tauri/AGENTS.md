# Tauri Host

- When the host needs something the engine does not offer, add it to the
  engine's host interface; product rules stay in the engine.
- The webview can deliver requests out of order, so the frontend numbers each
  session and settings intent and `intent_order.rs` runs them in number order.
- `log_file.rs` writes `abb.log` in a folder named for the Tauri identifier,
  because a Finder-launched app has no readable stderr and a dev build must
  not append to the installed app's log. A launch appends, so a second copy
  keeps a running copy's log.
- Tauri runs `RunEvent::Opened` on the main thread with no Tokio runtime, and
  on a cold launch before `setup`; `opened_files` blocks on the async engine
  call and holds the paths until the engine starts.
- `Quit` and `FrontendLink` are managed before the engine starts, so the
  second-copy exit path and the hidden webview's commands never panic.
- The process exits before `Engine::shutdown` settles only when the user
  chooses Quit Now, or Quit Anyway with unsaved settings. One `QuitPhase`
  changes only in `Quit`'s methods, so a quit or close request while the
  prompt is open or during shutdown is held and opens nothing new.
- Tauri's default Quit item calls AppKit `terminate:`, which raises no event
  ABB can hold, so on macOS `take_over_quit_item` swaps in ABB's own item.
  Dock Quit and logout still take `terminate:`, because tao has no
  `applicationShouldTerminate:`; `settle_unheld_exit` runs shutdown there
  without a question.
- A command is thin: take the IPC inputs, call one `Engine` method, and return
  `CommandResult<T>`. The engine validates paths, metadata intent, output
  truth, and lifecycle.
- A new user action is a new engine intent variant, not a new command. A
  separate command serves only a read outside a snapshot or an owner that
  takes no intents yet.
- `open_preview_output` is the one session action with its own command,
  because opening needs a host capability and the webview must never name a
  path to open. An open failure returns the reply with a `Rejected` outcome,
  so its update still applies.
- After a new command or an engine behavior change, run
  `bun run bindings:generate`: `src/lib/generated/fixtures/` records real
  engine outcomes and the snapshot parts that changed.
- A test of product behavior, including what a command reaches, belongs with
  the engine owner that decides it.
- In `commands/session_invoke_tests.rs`, send a numbered intent only after the
  one before it: a gap stalls the test for `MISSING_INTENT_WAIT`, then the
  number is refused as late.
- The fixture check runs on macOS and Linux, so a value that differs between
  machines is pinned in `pin_machine_values`. Name a title in `scenarios.json`
  only after importing it, or the recorder panics.
