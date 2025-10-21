# UI Probe Spike Plan

## Goals
- Enable rich remote inspection of the Audiobook Boss UI without shipping debugging aids to production builds.
- Establish an automation-friendly path (Playwright traces) that mirrors the Tauri window experience.
- Document clear steps for requesting and interpreting human-provided UI artifacts (screenshots, traces, JSON probes).

## Milestones
1. **Baseline Assessment**  
   - Audit current `window.testCommands` helpers and existing debug hooks.  
   - Capture any gaps in documentation or environment setup that block UI probing.
2. **Instrumentation Design**  
   - Define a typed `UIProbe` interface (selectors, snapshots, layout/style hooks).  
   - Specify dev-only gating and bundler strategy to keep code out of production builds.
3. **Automation & Browser Preview Pathfinding**  
   - Prototype a Playwright smoke-test workflow against the web build (`npm run dev`).  
   - Enable a dev-mode “web preview” served via Vite that stubs required Tauri APIs so chrome-devtools MCP sessions can exercise the UI directly.  
   - Determine feasibility of attaching Playwright to the Tauri runtime for parity checks.
4. **Implementation Spike** *(future step)*  
   - Implement minimal probe and automation scaffolding once design is approved.  
   - Capture trace bundles and JSON outputs for review.
5. **Verification & Handoff**  
   - Define acceptance criteria, regression coverage, and documentation updates.  
   - Share testing playbook with maintainers for ongoing use.

## TODO Tracker
- [ ] Inventory existing debug/testing utilities (docs, code, scripts).  
- [ ] Draft `UIProbe` TypeScript API proposal (methods, return shapes, usage guidelines).  
- [ ] Identify environment flag or build-time guard for probe activation.  
- [ ] Sketch Playwright trace capture workflow (commands, artifact paths, retention policy).  
- [ ] List critical UI flows to cover in the spike (import, metadata edit, processing status, error modals).  
- [ ] Define data handoff expectations for remote reviewers (trace zip, screenshots, probe JSON).  
- [ ] Align on success criteria and exit conditions before coding.
- [ ] Design browser-only mock layer for Tauri IPC (`window.__TAURI__`, `invoke`) to keep the web UI functional in dev preview mode.  
- [ ] Document chrome-devtools MCP workflow (dev server command, host/port, authentication expectations).

## Assumptions & Risks
- Tauri WebView exposes enough APIs to mirror behaviors observed in the browser build.  
- Playwright integrations can run in CI or a local scripted environment without bundling the desktop app.  
- Additional dependencies (Playwright, axe-core) will be introduced in later phases, pending approval.
- Dev Vite server can be exposed on `localhost` for MCP chrome-devtools access without violating sandbox constraints.  
- Browser preview mode remains close enough to the Tauri experience that findings translate; discrepancies (WebKit vs Chromium) are documented when discovered.

## Human Feedback Workflow (Planned)
1. Developer runs the probe/automation scripts locally and gathers artifacts (Playwright trace, screenshots, probe JSON).  
2. Artifacts and a short scenario summary are shared with the reviewer (attach to issue/PR or CLI upload).  
3. Reviewer inspects the trace bundle locally, annotates findings, and cross-references probe outputs for state verification.  
4. Follow-up inputs (e.g., additional user-driven interactions) are requested via scripted prompts to ensure reproducibility.  
5. Outcome and any action items feed back into the TODO tracker above.
