# Implementation Plan: The "Force Multiplier" (Tauri Bridge)

This document outlines the plan to implement a `TauriBridge` layer, enabling the frontend to run fully in a standard web browser by mocking Tauri's backend APIs.

## Impact Analysis

### 1. Surfaces Touched
The following files currently import directly from `@tauri-apps/*` and will require refactoring:

| File | API Usage | Change Required |
| :--- | :--- | :--- |
| `src/main.ts` | `invoke` (many commands) | Replace import with `bridge` |
| `src/ui/fileImport.ts` | `invoke`, `listen`, `open` | Replace imports with `bridge` |
| `src/ui/outputPanel.ts` | `open` | Replace import with `bridge` |
| `src/ui/coverArt.ts` | `invoke`, `open` | Replace imports with `bridge` |
| `src/ui/fileList/actions.ts` | `invoke` | Replace import with `bridge` |
| `src/ui/statusPanel/logic.ts` | `invoke`, `listen`, `openExternal` | Replace imports with `bridge` |

### 2. Why This Change?
Currently, the frontend is tightly coupled to the Tauri runtime.
-   **Problem**: In a browser (e.g., `npm run dev`), calls to `window.__TAURI__` fail, crashing the app or breaking features.
-   **Solution**: A "Bridge" layer that detects the environment.
    -   **Tauri Environment**: Passes calls through to the real Tauri APIs.
    -   **Browser Environment**: Intercepts calls and returns mock data (e.g., "Fake File Analysis", "Simulated Progress").

### 3. Impact on You (The Solo Dev)
*   **Velocity 🚀**: You can develop UI features (animations, layouts, complex forms) in Chrome/Firefox with hot-reload, without waiting for Rust builds or dealing with Tauri window quirks.
*   **Reliability 🛡️**: Automated agents (like me) can run end-to-end tests on the UI logic. We can "upload" a file and verify the "Processing" UI state without needing the actual backend.
*   **Maintenance 🔧**:
    *   *Cost*: You must maintain the "Mocks". If you change a Rust command signature, you must update the mock data.
    *   *Mitigation*: Keep mocks simple. They don't need to be perfect, just sufficient to unblock UI states.

### 4. Impact on Agents
*   **Visibility**: Agents can "see" the entire application flow, not just static pages.
*   **Verification**: We can verify that clicking "Process" triggers the correct UI transitions, error handling, and success states.

### 5. Production & Performance Impact
*   **Bundle Size**: We will use `import.meta.env.DEV` to guard the mock logic. Vite's build process will "tree-shake" (remove) the mock code entirely from the production build.
*   **Runtime Overhead**: The bridge introduces a single boolean check (`if (isTauri)`) per API call. This is nanosecond-level overhead—completely imperceptible to users.
*   **Safety**: The production app will contain *zero* mock logic, ensuring no accidental "fake" behavior in the release.

---

## Implementation Plan

### Phase 1: The Bridge Foundation

#### [NEW] [src/lib/bridge.ts](file:///Users/jstar/Projects/audiobook-boss/src/lib/bridge.ts)
-   **Purpose**: The single source of truth for API calls.
-   **Logic**:
    ```typescript
    // Check if we are in a Tauri environment
    const isTauri = !!(window as any).__TAURI_INTERNALS__;

    // Dynamic import for mocks to ensure tree-shaking in production
    const getMocks = async () => {
        if (import.meta.env.DEV) {
            return await import('./mocks');
        }
        throw new Error('Mocks not available in production');
    };

    export const bridge = {
        invoke: async (cmd, args) => {
            if (isTauri) return tauriInvoke(cmd, args);
            if (import.meta.env.DEV) return (await getMocks()).mockInvoke(cmd, args);
            console.warn('Tauri not detected and not in DEV mode');
        },
        // ... similar logic for open, listen
    };
    ```

#### [NEW] [src/lib/mocks.ts](file:///Users/jstar/Projects/audiobook-boss/src/lib/mocks.ts)
-   **Purpose**: specific mock implementations to keep `bridge.ts` clean.
-   **Content**:
    -   `mockAnalyzeAudioFiles`: Returns a valid `FileListInfo`.
    -   `mockProcessAudiobook`: Simulates a 5-second progress bar (0% -> 100%) via events.

### Phase 2: Documentation & Standards

#### [MODIFY] [AGENTS.md](file:///Users/jstar/Projects/audiobook-boss/AGENTS.md)
-   Add **"Mock Maintenance"** rule under `Testing & Verification`.
-   *Rule*: "When changing Rust command signatures, you MUST update the corresponding mock in `src/lib/mocks.ts` to keep the browser dev environment functional."

### Phase 3: Refactoring (The "Search & Replace")

#### [MODIFY] [src/main.ts](file:///Users/jstar/Projects/audiobook-boss/src/main.ts)
-   Replace `@tauri-apps/api/core` imports with `../lib/bridge`.

#### [MODIFY] [src/ui/fileImport.ts](file:///Users/jstar/Projects/audiobook-boss/src/ui/fileImport.ts)
-   Update imports.
-   Ensure `mockOpen` returns a fake file path so the UI thinks a file was selected.

#### [MODIFY] [src/ui/outputPanel.ts](file:///Users/jstar/Projects/audiobook-boss/src/ui/outputPanel.ts)
-   Update imports.

#### [MODIFY] [src/ui/coverArt.ts](file:///Users/jstar/Projects/audiobook-boss/src/ui/coverArt.ts)
-   Update imports.

#### [MODIFY] [src/ui/fileList/actions.ts](file:///Users/jstar/Projects/audiobook-boss/src/ui/fileList/actions.ts)
-   Update imports.

#### [MODIFY] [src/ui/statusPanel/logic.ts](file:///Users/jstar/Projects/audiobook-boss/src/ui/statusPanel/logic.ts)
-   Update imports.
-   Ensure the mock `process` command emits `processing-progress` events so the UI updates.

## Verification Plan

### Automated Verification
1.  **Browser Agent Test**:
    -   Open `http://localhost:1420/`.
    -   Click "Browse..." (File Import).
    -   **Expectation**: Instead of an error, the UI should show "Fake File.mp3" (from the mock).
    -   Click "Process".
    -   **Expectation**: The progress bar should animate from 0% to 100% (driven by the mock).

### Manual Verification
1.  **Tauri App Test**:
    -   Run `npm run tauri dev`.
    -   Verify that the *real* app still works (files are actually analyzed, processing actually happens).
    -   This confirms the "passthrough" mode works.
