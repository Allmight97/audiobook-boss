# Max See & Touch Plan

## Desired Outcomes

- Enable MCP-compatible agents to capture screenshots, inspect DOM, and issue input events against the live Tauri WKWebView during dev runs.
- Keep exposure strictly dev-only with runtime gating, minimal attack surface, and an opt-in kill switch.
- Provide stable UI affordances (`data-testid`, agent helpers) so scripted interactions stay deterministic.

## Implementation Steps

- **add-plugin**: Add the `tauri-plugin-mcp` dependency in `src-tauri/Cargo.toml`, enable it behind `#[cfg(debug_assertions)]`, and wire `plugin(mcp::init(...))` inside `src-tauri/src/main.rs` with a guarded socket/TCP configuration.
- **dev-security**: Define dev-only transport settings (`config/dev/mcp.json` or env vars), inject them via `tauri.conf.json` build config, enforce allowlisted tools, unique socket path, and an environment kill switch.
- **agent-surface**: In the TS frontend (`src/ui/...`), add durable `data-testid` attributes to key controls and expose a `window.__AGENT__` helper with safe intents (`startEncode`, `openProject`, `getState`).
- **docs-ops**: Document setup in `docs/development/mcp-agent.md`: how to start the plugin server, run the Node MCP bridge from the plugin repo, connect agents (IPC, TCP), and security notes.

## Optional Enhancements

- Add post-start log message confirming MCP endpoint and active safeguards.
- Provide a small smoke script (Node CLI) under `scripts/mcp-smoke.ts` that pings the MCP endpoint to validate readiness.

## Todos

- add-plugin: Wire plugin dependency and initialization for debug builds only.
- dev-security: Configure transport, allowlist, and kill switch safeguards.
- agent-surface: Add selectors and `window.__AGENT__` helpers for deterministic automation.
- docs-ops: Write developer setup and usage instructions.