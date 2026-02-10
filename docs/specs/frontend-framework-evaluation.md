# Frontend Framework Evaluation

**Date**: 2026-02-10
**Updated**: 2026-02-10 (added Electron vs Tauri analysis)

**Context**: Research exploring the co-location pattern (styles + logic + markup in one component), evaluating frontend framework options for Audiobook Boss, adding Opus Agent (frontend design-focused collaborator) to the decision surface, and critically evaluating the Tauri vs Electron foundation decision.

**⚠️ CRITICAL DECISION POINT**: This document now includes a fundamental architectural choice that must be resolved BEFORE framework/type-safety decisions: **Should this app be rebuilt on Electron instead of Tauri?**

---

## Executive Summary

**The original questions**:
1. What would adopting a frontend framework (React, Svelte, Vue, Solid) mean for this project?
2. What's the value of "co-location" (styles + logic + markup in one file)?
3. Should we add full-stack type safety (tauri-specta)?

**The new question that supersedes all others**:
4. **Is Tauri the right foundation, or should we rebuild on Electron?**

**The answers**:
1. Co-location requires a **component model** to fully realize (Svelte 5 is lowest-friction migration)
2. Full-stack type safety (tauri-specta) is low-risk, high-value, and helps with cross-platform
3. **BUT**: None of this matters if the Tauri foundation is fundamentally wrong for AI-assisted development and cross-platform visual consistency

**Recommendation (CONDITIONAL)**:
- **IF staying on Tauri**: Svelte 5 + tauri-specta is the right path
- **IF migrating to Electron**: The framework/type-safety decisions remain valid, but the Rust backend strategy changes significantly

**Action required**: Read the "Electron vs Tauri: The Foundation Decision" section below before proceeding with any other architectural changes.

---

---

## ⚠️ ELECTRON VS TAURI: THE FOUNDATION DECISION

**Context**: Theo (t3.gg) recently made a strong public argument against Tauri in favor of Electron, specifically calling out developers who don't seriously consider Electron as either "ignorant juniors" or "biased seniors." He highlights [lossless-cut](https://github.com/mifi/lossless-cut) (an Electron app wrapping FFmpeg for video editing) as a proof point that Electron + native bindings works well for media apps.

This is not a theoretical debate. **Your repo is Tauri v2 with Rust backend wrapping ffmpeg-next**. If Theo's critique is valid, you may be building on the wrong foundation for AI-assisted cross-platform development.

### Theo's Core Arguments (Paraphrased from Video)

#### Against Tauri:

**1. Inconsistent Rendering (The "Browser Wars" Problem)**
- Tauri uses OS-native webviews: **WebKit (macOS) → WebView2/Chromium (Windows) → WebKitGTK (Linux)**
- This reintroduces platform-specific CSS/JS bugs (e.g., a layout bug exists on Windows but not macOS)
- No "write once, run everywhere" guarantee -- you must test on all platforms and fix engine-specific quirks

**2. AI Debugging Difficulty**
- AI agents (Claude, GPT, etc.) are trained primarily on **Chromium/modern browser behavior**
- When an AI attempts to fix a WebKitGTK-specific bug, it often assumes standard Chrome behavior
- Debugging engine-specific quirks (e.g., CSS Grid implementation differences, older Safari WebKit features missing) is **much harder for AI** than debugging a standardized Chromium environment
- **This is the killer argument for AI-assisted solo dev**: If your AI collaborator can't reliably debug cross-platform rendering, you're back to manual platform testing

**3. Manual Bindings Friction**
- Binding Rust to JS requires boilerplate (even with Tauri v2 improvements)
- Context-switching between Rust backend and JS/TS frontend is cognitively expensive
- Electron's Node.js backend lets you stay in the same language ecosystem (though Theo acknowledges you can use native modules in Electron too)

#### For Electron:

**1. Chromium Consistency**
- You ship **the exact same browser engine** to every user (Chromium is bundled)
- If it works on your macOS dev machine, it works on Windows/Linux (modulo OS-level file dialog/menu differences, which both Electron and Tauri have)
- **Zero "browser war" issues** -- CSS Grid, Flexbox, Web APIs all behave identically

**2. AI Debugging Advantage**
- AI agents can reliably fix bugs because Chromium behavior is standard, well-documented, and matches their training data
- No need to distinguish "is this a WebKitGTK bug or a logic bug?" -- if it breaks, it's your code or Chromium (and Chromium bugs are rare and widely known)

**3. Performance Reality Check**
- Theo argues that while Electron uses more RAM (~50-100 MB extra for bundled Chromium), the "bloat" is overstated on modern hardware
- Optimizing a native Swift/Rust UI to match Chromium's battle-tested text rendering, font shaping, and hardware acceleration often results in **worse perf and more bugs** than just using Electron
- Quote: "Good luck beating Chromium's text rendering performance in a native UI without hiring a team of rendering engineers"

**4. Mature Ecosystem**
- Electron has been production-stable since 2013 (Atom editor, VS Code, Slack, Discord, Figma, Notion, Obsidian)
- Tauri v1 shipped in 2022, v2 in 2024 (much younger, smaller ecosystem, fewer solved problems)
- FFmpeg bindings exist for Electron (via Node.js native modules or WASM), proven in lossless-cut

#### Theo's Recommendation:
- **Electron by default** unless you have a specific reason to go native (e.g., battery life on mobile, ultra-low RAM constraints)
- Tauri is "fine for simple apps" but not ideal for cross-platform apps that need AI-assisted debugging

### Counter-Arguments (Pro-Tauri)

**1. Bundle Size & Startup Time**
- Tauri apps are **significantly smaller** (5-10 MB vs 100+ MB for Electron due to bundled Chromium)
- Faster cold start (no Chromium initialization)
- **Counter-counter**: Theo argues this matters less on modern hardware; users care more about "does it work reliably?" than "is the .dmg 95 MB smaller?"

**2. RAM Usage**
- Tauri uses **50-200 MB less RAM** (no separate Chromium instance, just OS webview)
- **Counter-counter**: Modern machines have 16-32 GB RAM; an extra 100 MB is negligible for a desktop app

**3. Security Posture**
- Tauri's architecture (Rust backend, minimal JS surface, no Node.js) has a **smaller attack surface**
- **Counter-counter**: Electron's security model (contextBridge, sandboxed renderer) is battle-tested and secure if used correctly. Most Electron security issues are developer misconfigurations, not framework flaws.

**4. Native Feel**
- Tauri can use **native OS UI elements** (file dialogs, menus, system tray) more easily than Electron
- **Counter-counter**: Electron also provides native dialogs/menus via `electron` APIs. The "native feel" advantage is minimal in practice.

**5. Rust Backend Leverage**
- If your app needs **high-performance native code** (e.g., audio processing with ffmpeg-next), Rust is a better choice than Node.js C++ addons
- **Counter-counter**: Electron can call Rust via WASM or native Node modules (N-API). lossless-cut proves FFmpeg + Electron works fine.

### The AI-Assisted Development Lens (CRITICAL)

**Theo's strongest point for YOUR use case**: AI agents struggle with engine-specific bugs.

**Scenario 1 (Tauri)**: Your AI spots a CSS layout bug in your app.
- **On macOS** (WebKit): works fine
- **On Windows** (WebView2/Chromium): works fine
- **On Linux** (WebKitGTK 2.38): broken due to an older Flex implementation

**What happens**:
1. AI proposes a fix assuming modern Chrome behavior
2. Fix works on macOS/Windows but fails on Linux
3. You (human) must debug WebKitGTK docs, test on Linux VM, identify the quirk
4. AI cannot reliably help because it doesn't have WebKitGTK 2.38 behavior in training data
5. You're back to manual platform testing

**Scenario 2 (Electron)**: Same CSS layout bug.
- If it breaks, it's broken **everywhere** (same Chromium engine)
- AI proposes a fix, you test on macOS, it works
- High confidence it also works on Windows/Linux (modulo OS-level dialogs, which both frameworks have)

**This is Theo's core thesis**: For AI-assisted solo dev, **predictable engine behavior > raw performance**.

### What This Means for Audiobook Boss

**Current state**:
- Tauri v2 app with Rust backend (ffmpeg-next)
- ~4,800 LOC TypeScript frontend
- ~5,000+ LOC Rust backend (audio processing, metadata, commands)
- macOS-only, but Windows/Linux planned

**If you stay on Tauri**:
- Must add **rigorous cross-platform visual testing** (screenshot diffs on Windows/Linux VMs, not just macOS)
- AI agents (Claude, Opus) will struggle with WebKitGTK-specific bugs (you'll need to manually test Linux)
- Bundle size/RAM advantages remain (5-10 MB app, 50-100 MB RAM savings)
- Rust backend stays as-is (no migration cost)

**If you migrate to Electron**:
- **Huge migration cost**: rewrite all 15+ `#[tauri::command]` handlers as Electron IPC (ipcMain/ipcRenderer)
- Decide: keep Rust backend (call via N-API or WASM), or rewrite in Node.js?
- Gain: Chromium consistency, AI-friendly debugging, proven FFmpeg integration (lossless-cut proof point)
- Lose: bundle size (100+ MB app), RAM usage (extra 50-100 MB), Rust-native performance

### Migration Cost Estimate (Tauri → Electron)

**If keeping Rust backend** (via N-API or WASM):
- **2-3 weeks work** (high estimate for a major rewrite)
  - Replace Tauri IPC with Electron IPC (ipcMain/ipcRenderer)
  - Replace `bridge.ts` with Electron's contextBridge pattern
  - Rewrite Rust command handlers as N-API exports or WASM modules
  - Test cross-platform (Windows/Linux VMs)
  - Fix engine-specific quirks (unlikely if staying on Chromium)

**If rewriting backend in Node.js** (using fluent-ffmpeg or similar):
- **4-6 weeks work** (full rewrite)
  - Rewrite ~5,000 LOC Rust backend in TypeScript/Node.js
  - Replace ffmpeg-next with fluent-ffmpeg or node-fluent-ffmpeg
  - Replace mp4ameta with mp4-parser or similar JS library
  - Lose: Rust's type safety, performance, memory safety
  - Gain: single-language codebase (TS everywhere)

**Risk**: High. This is a **foundation rewrite**, not an incremental change. No rollback path without weeks of git revert churn.

### The lossless-cut Proof Point

[lossless-cut](https://github.com/mifi/lossless-cut) (11k+ GitHub stars, actively maintained):
- **Electron app** wrapping FFmpeg CLI (via shell exec, not native bindings)
- Supports Windows/Linux/macOS with consistent UI/UX
- Handles **video editing** (heavier workload than audiobook metadata/encoding)
- Single developer (Mikael Finstad), AI-assisted (proven in recent commits)

**Key takeaways**:
1. Electron + FFmpeg is a proven pattern for media apps
2. You don't need Rust/native bindings -- shell exec FFmpeg CLI works fine (though you lose type safety and direct control)
3. Cross-platform "just works" with Chromium consistency

**Counter-takeaway**: lossless-cut uses FFmpeg CLI, not native bindings. Your repo uses ffmpeg-next (Rust bindings) for finer control and better error handling. Migrating to Electron + FFmpeg CLI would lose that control.

### Decision Matrix

| Factor | Tauri (Current) | Electron (Migrate) |
|--------|----------------|-------------------|
| **Bundle size** | 5-10 MB | 100-150 MB |
| **RAM usage** | 50-100 MB | 150-250 MB |
| **Cross-platform rendering** | ❌ Engine-specific quirks (WebKit, WebView2, WebKitGTK) | ✅ Chromium everywhere |
| **AI debugging ease** | ❌ Hard (engine-specific bugs confuse AI) | ✅ Easy (standard Chromium behavior) |
| **FFmpeg integration** | ✅ Native Rust bindings (ffmpeg-next) | ⚠️ N-API/WASM (keep Rust) or fluent-ffmpeg (rewrite) |
| **Migration cost** | $0 (already done) | 2-6 weeks work (high risk) |
| **Ecosystem maturity** | ⚠️ Tauri v2 (2024), smaller community | ✅ Electron (2013), massive ecosystem |
| **Security posture** | ✅ Smaller attack surface (no Node.js) | ⚠️ Requires careful contextBridge usage |
| **Startup time** | ✅ Fast (OS webview) | ⚠️ Slower (Chromium init) |
| **Solo dev + AI** | ❌ Manual platform testing required | ✅ AI can debug reliably |

### Honest Assessment (From a Senior Engineer Who Respects Theo's Opinion)

**Theo is right about one thing**: If you're a solo developer relying on AI agents for cross-platform support, **Chromium consistency is a huge advantage**. The WebKitGTK debugging scenario is real and painful.

**But**: You've already invested ~1 year building on Tauri + Rust. The sunk cost fallacy is real, but so is **momentum**. Rebuilding on Electron resets that momentum.

**The pragmatic question**: How much cross-platform visual testing pain are you willing to tolerate vs. how much migration risk can you stomach?

### Three Paths Forward

#### Path A: Stay on Tauri, Accept Trade-offs
**Decision**: Tauri's bundle size / RAM advantages matter more than Chromium consistency.

**What you must do**:
1. **Rigorous cross-platform visual testing**: Set up Windows/Linux VMs (or use GitHub Actions with screenshot diffing)
2. **Document engine quirks**: When you hit a WebKitGTK-specific bug, document it in `CLAUDE.md` so AI agents can reference it
3. **Limit CSS complexity**: Stick to battle-tested Tailwind utilities, avoid cutting-edge CSS features that might have engine-specific bugs
4. **Accept AI debugging limits**: Some bugs will require manual Linux testing; AI can't always help

**Pros**: No migration cost, keep Rust backend, bundle size / RAM advantages
**Cons**: Cross-platform pain, AI debugging limits, younger ecosystem

---

#### Path B: Migrate to Electron + Keep Rust Backend
**Decision**: Chromium consistency and AI debugging ease justify migration cost.

**What you must do**:
1. **Spike: Electron + N-API/WASM proof-of-concept** (1 week): Can you call ffmpeg-next from Electron? Is performance acceptable?
2. **Migrate IPC layer** (1-2 weeks): Replace Tauri commands with Electron ipcMain/ipcRenderer
3. **Test cross-platform** (1 week): Verify Windows/Linux work with same codebase
4. **Document migration learnings** (1 day): What broke, what was easy, lessons for next time

**Pros**: Chromium consistency, AI debugging ease, mature ecosystem, keep Rust performance
**Cons**: 2-3 weeks migration cost, bundle bloat, RAM increase, Rust<->Node.js bridge complexity

---

#### Path C: Migrate to Electron + Rewrite Backend in Node.js
**Decision**: Single-language codebase (TS everywhere) is worth the full rewrite.

**What you must do**:
1. **Spike: Node.js FFmpeg integration** (1 week): Can fluent-ffmpeg handle your encoding needs? What about metadata?
2. **Rewrite audio processing** (2-3 weeks): Port ffmpeg-next logic to fluent-ffmpeg or shell-exec FFmpeg CLI
3. **Rewrite metadata handling** (1 week): Port mp4ameta logic to mp4-parser or similar JS library
4. **Test cross-platform** (1 week): Same as Path B
5. **Migrate IPC layer** (easier than Path B, since backend is now TS)

**Pros**: Single language (TS), simpler stack, easier AI collaboration (no Rust<->TS context switch)
**Cons**: 4-6 weeks migration cost, lose Rust type safety / performance / memory safety, bundle bloat, RAM increase

---

### Recommendation (Conditional on Risk Tolerance)

**If you're risk-averse (ship-it mode)**: **Path A (Stay on Tauri)**
- You're 1 year in, close to launch, cross-platform testing is manageable
- Add rigorous visual testing (screenshot diffs), document engine quirks for AI
- Revisit Electron post-launch if cross-platform bugs become unbearable

**If you're risk-tolerant (willing to reset timeline)**: **Path B (Electron + Keep Rust)**
- Chromium consistency and AI debugging ease are worth 2-3 weeks migration
- Keep your Rust backend investment (ffmpeg-next, mp4ameta)
- lossless-cut proves Electron + native bindings works for media apps

**If you want to simplify long-term**: **Path C (Electron + Node.js)**
- Single-language codebase is easier for AI agents (no Rust<->TS context switch)
- But: lose Rust performance / safety, and 4-6 weeks is a major timeline reset

**My gut** (as a senior engineer who respects Theo but also respects your 1-year investment): **Stay on Tauri for now (Path A), but plan for Electron migration post-launch if cross-platform pain becomes real**. Don't let sunk cost blind you, but also don't throw away working code based on a YouTube video. Test the pain threshold first.

---

## Additional Consideration: Opus Agent

Opus Agent is now in scope as a design-focused collaborator for production UI quality and consistency.

This changes the decision criteria from "framework ergonomics only" to a dual-track evaluation:

1. **Engineering track**: runtime model fit, migration cost, bundle/perf profile, IPC adaptation cost
2. **Design track**: component-level design consistency, token governance, style drift prevention, predictable UX evolution

**Implication**: The winning option is not just the fastest technical migration. It is the option that best supports a durable design system and stable component contracts while the app continues to evolve.

---

## Current Stack Overview

### Architecture
- **No framework** -- vanilla TypeScript with imperative DOM manipulation
- **Single static HTML file** (`index.html`, 961 lines) defines the full layout
- **TypeScript modules** (`src/ui/`, ~4,800 LOC) attach behavior via `getElementById`, `addEventListener`, `classList.toggle`
- **State management**: Module-scoped `let` variables with exported getters/setters (no centralized store, no reactive system)

### Styling Layers (Three-Layer Hybrid)

| Layer | Location | Role |
|-------|----------|------|
| **Static Tailwind utilities** | `index.html` | `flex items-center gap-2 mb-2` |
| **Custom CSS** | `src/styles.css` (1,613 lines) | CSS custom properties (design tokens), BEM-ish component classes (`.panel`, `.file-list-item`, `.cover-art-area`) |
| **Dynamic classes** | TS template literals | `className = \`file-list-item ${file.isValid ? "valid" : "invalid"}\`` |

**Key insight**: We already have Tailwind v4 installed and active, but most styling work is carried by the monolithic CSS file.

### Component Structure (11 Logical Components)

1. **FileImport** (178 LOC) -- drag/drop + file dialog
2. **FileList** (620 LOC, 6 files) -- file list + sort + reorder + selection
3. **FileList MetadataPanel** (222 LOC) -- file properties inspector
4. **MetadataForm** (445 LOC) -- form read/write/dirty-tracking
5. **CoverArt** (436 LOC) -- cover art load/display/clear/URL/drag
6. **MetadataLookup** (582 LOC) -- online search modal
7. **TagPreview** (113 LOC) -- tag preview grid
8. **OutputPanel** (560 LOC, 5 files) -- output dir + naming + estimated size
9. **EncoderPanel** (470 LOC, 5 files) -- encoder settings + persistence
10. **StatusPanel** (1,300 LOC, 12 files) -- progress bar + job list + processing orchestration (most complex)
11. **JobControls** (149 LOC) -- merge toggle + concurrency selector

Several modules are **already decomposed** into `index.ts` / `state.ts` / `dom.ts` / `events.ts` / `logic.ts` patterns, which creates natural seams for a framework migration.

---

## The Co-location Gap

**Current reality**: Styles, logic, and markup live in three separate places:

| Concern | Lives in | Example |
|---------|----------|---------|
| Layout/markup | `index.html` (961 lines) | `<div class="panel input-panel">` |
| Behavior/state | `src/ui/*.ts` (~4,800 LOC) | `getElementById`, `classList.toggle` |
| Appearance | `src/styles.css` (1,613 lines) | `.panel { border: 1px solid var(--border-primary) }` |

**The co-location dream**: In a component framework with Tailwind, all three converge:

```svelte
<!-- Svelte example (NOT our current codebase) -->
<script lang="ts">
  let isValid = $state(true);
</script>

<div class={`panel p-4 ${isValid ? "border-green-500" : "border-red-500"}`}>
  <button class="btn-pill" onclick={() => isValid = !isValid}>Toggle</button>
</div>

<style>
  /* Optional scoped CSS for complex component-specific needs */
</style>
```

**Why it matters**: Discoverability, refactoring safety, and cognitive load. When you delete a component file, all its styles, logic, and markup go with it. No orphaned CSS classes, no stale event listeners.

**Without a framework**, pushing deeper into Tailwind would mean:
- Converting `.panel`, `.file-list-item`, etc. into chains of utility classes directly in `index.html` and template literals
- `styles.css` shrinks (good), but `index.html` class attributes grow (trade-off)
- Still have the three-file separation (HTML / TS / CSS), just with less in the CSS file

**The full co-location benefit requires a component model** (React, Svelte, Vue, Solid, etc.) where markup + logic + styles live in one file per UI concern.

---

## Framework Comparison

All four frameworks are Tauri-compatible (Tauri 2.x is framework-agnostic by design). Since we already use Vite, any Vite-compatible framework slots in with minimal config changes.

With Opus Agent included, we should additionally prefer frameworks that make design-system enforcement easy (tokens, variants, scoped/owned component styles, and low ambiguity between utility classes and component classes).

### Svelte 5

| Dimension | Assessment |
|-----------|-----------|
| **Runtime model** | Compiler-first. No virtual DOM. Compiles to imperative DOM operations -- **exactly what we're writing by hand**. |
| **Bundle overhead** | ~28 KB (framework compiles away, only runtime helpers remain) |
| **DX** | `.svelte` files co-locate `<script>` + markup + `<style>`. Runes (`$state`, `$derived`, `$effect`) make reactivity explicit. |
| **Migration cost** | **Lowest**. Our `state.ts` → `$state` runes (nearly 1:1 mapping). Our `dom.ts` files → Svelte template bindings. |
| **TypeScript** | First-class, though some devs report strongly-typed props became more verbose in Svelte 5. |
| **Ecosystem** | Smaller than React/Vue. Fewer pre-built component libraries. Less of a concern for a solo desktop app. |
| **Tailwind v4** | Native support via Vite plugin. CLI can scaffold with Tailwind v4 out of the box. |
| **Maturity** | Stable and production-ready (Svelte 5 + Runes stable as of late 2024). |
| **Desktop fit** | **Excellent**. Compiles to the same imperative code we're already writing. DX upgrade is about eliminating boilerplate (`getElementById`, manual event listeners, manual DOM updates) while keeping the same runtime behavior. |

**State mapping example**:
```ts
// Current (fileList/state.ts)
let currentFileList: AudioFile[] = [];
export function setCurrentFileList(files: AudioFile[]): void {
  currentFileList = files;
}

// Svelte 5 equivalent
let currentFileList = $state<AudioFile[]>([]);
// Reactivity is automatic; no setter needed
```

**Co-location example**:
```svelte
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  let files = $state<AudioFile[]>([]);
  let isProcessing = $state(false);

  async function handleProcess() {
    isProcessing = true;
    await invoke('process_audiobook_files_v2', { files });
    isProcessing = false;
  }
</script>

<div class="panel p-4">
  <button
    class="btn-pill btn-pill-primary"
    onclick={handleProcess}
    disabled={isProcessing}
  >
    {isProcessing ? 'Processing...' : 'Process'}
  </button>
</div>

<style>
  /* Optional scoped CSS for this component only */
</style>
```

### SolidJS

| Dimension | Assessment |
|-----------|-----------|
| **Runtime model** | Fine-grained reactivity with signals. No virtual DOM. Direct DOM updates. JSX syntax. |
| **Bundle overhead** | ~30 KB (12 KB runtime + 10 KB router + 8 KB state) |
| **DX** | JSX with signals (`createSignal`, `createEffect`, `createMemo`). Looks like React but behaves fundamentally differently -- **components run once**, not on every render. |
| **Migration cost** | Moderate. JSX is familiar if you know React, but the "runs once" mental model requires relearning. Our `state.ts` patterns map well to signals. |
| **TypeScript** | First-class (written in TS). |
| **Ecosystem** | Smaller than Svelte's, which is already smaller than React/Vue. Component libraries exist but are fewer. |
| **Tailwind v4** | Good support via Vite plugin. |
| **Maturity** | **1.x is stable**. However, **2.0 is still experimental** (no release date) with breaking changes to Resources, async behavior, and store APIs. |
| **Desktop fit** | Strong. The fine-grained reactivity is ideal for real-time progress updates (our status panel). **Performance ceiling is the highest** of any framework in benchmarks. |

**Trade-off**: The pending 2.0 migration and the steeper learning curve (the "runs once" gotchas like destructuring props breaking reactivity) make it a riskier bet for a project in ship-it mode.

**State mapping example**:
```tsx
// Current (fileList/state.ts)
let currentFileList: AudioFile[] = [];
export function setCurrentFileList(files: AudioFile[]): void {
  currentFileList = files;
}

// SolidJS equivalent
const [currentFileList, setCurrentFileList] = createSignal<AudioFile[]>([]);
```

**Co-location example**:
```tsx
function ProcessButton() {
  const [isProcessing, setIsProcessing] = createSignal(false);

  async function handleProcess() {
    setIsProcessing(true);
    await invoke('process_audiobook_files_v2', { files });
    setIsProcessing(false);
  }

  return (
    <div class="panel p-4">
      <button
        class="btn-pill btn-pill-primary"
        onClick={handleProcess}
        disabled={isProcessing()}
      >
        {isProcessing() ? 'Processing...' : 'Process'}
      </button>
    </div>
  );
}
```

### Vue 4

| Dimension | Assessment |
|-----------|-----------|
| **Runtime model** | Optimized virtual DOM with async batching. Composition API (`ref`, `computed`, `watch`) is clean. |
| **Bundle overhead** | ~58 KB (38 KB runtime + 12 KB router + 8 KB state) |
| **DX** | Single-file components (`.vue`) are the gold standard for co-location. TypeScript support is strong (especially with `<script setup lang="ts">`). |
| **Migration cost** | Moderate. Vue's `ref()` / `reactive()` map conceptually to our `state.ts` patterns. Templates are HTML-like, so our existing HTML could be chunked into `.vue` files. |
| **TypeScript** | Strong support, especially with `<script setup>`. |
| **Ecosystem** | Strong. More mature than Svelte/Solid, less than React. |
| **Tailwind v4** | Good support via Vite plugin. |
| **Maturity** | Stable and production-ready. |
| **Desktop fit** | Good. Vue's incremental adoption story is real -- you can adopt it component-by-component. The Composition API is lean and explicit. |

**Trade-off**: Virtual DOM overhead (irrelevant at our scale), larger bundle than Svelte/Solid.

**State mapping example**:
```ts
// Current (fileList/state.ts)
let currentFileList: AudioFile[] = [];
export function setCurrentFileList(files: AudioFile[]): void {
  currentFileList = files;
}

// Vue equivalent
const currentFileList = ref<AudioFile[]>([]);
// Reactivity is automatic; no setter needed
```

**Co-location example**:
```vue
<script setup lang="ts">
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';

const isProcessing = ref(false);

async function handleProcess() {
  isProcessing.value = true;
  await invoke('process_audiobook_files_v2', { files });
  isProcessing.value = false;
}
</script>

<template>
  <div class="panel p-4">
    <button
      class="btn-pill btn-pill-primary"
      @click="handleProcess"
      :disabled="isProcessing"
    >
      {{ isProcessing ? 'Processing...' : 'Process' }}
    </button>
  </div>
</template>

<style scoped>
  /* Optional scoped CSS for this component only */
</style>
```

### React 19

| Dimension | Assessment |
|-----------|-----------|
| **Runtime model** | Virtual DOM diffing. Overhead is real but irrelevant for our UI complexity. |
| **Bundle overhead** | ~72 KB (45 KB runtime + 15 KB router + 12 KB state) -- largest of the four. |
| **DX** | Hooks are mature. TypeScript support is excellent. Massive ecosystem. |
| **Migration cost** | **Highest**. Our imperative DOM code maps poorly to React's declarative model. Every `getElementById` + `innerHTML` becomes a component with state hooks. Our existing `state.ts`/`dom.ts` split does not carry over -- React merges them. |
| **TypeScript** | Excellent support. |
| **Ecosystem** | Massive. The most component libraries, tutorials, community support. |
| **Tailwind v4** | Good support (note: React uses `className` instead of `class`, which is a papercut when migrating existing HTML). |
| **Maturity** | Stable and production-ready. |
| **Desktop fit** | Overkill. React's value propositions (ecosystem size for hiring, SSR/Next.js, community support for large teams) don't serve a solo desktop app. |

**Trade-off**: Highest migration cost, largest bundle, least architectural alignment with our current imperative patterns.

---

## Migration Scope Analysis

Our existing architecture has a **huge advantage** -- we already separate `state.ts` / `dom.ts` / `events.ts` / `logic.ts` in decomposed modules like `statusPanel/`, `fileList/`, `outputPanel/`, `encoderPanel/`. This creates natural seams for migration.

### What Would Change (LOC Breakdown)

| Category | Approx LOC | % of Frontend | What Happens |
|----------|-----------|---------------|-------------|
| **Full rewrite** (DOM/templates/events) | 2,300 | 48% | All `dom.ts` files (fileList/dom, outputPanel/dom, statusPanel/dom, encoderPanel/dom) become component templates. All `addEventListener` wiring → framework directives (`on:click`, `@click`, `onClick`). The 961-line `index.html` dissolves into ~11 component files. |
| **Adapt** (wrap in reactive primitives) | 1,100 | 23% | State modules (fileList/state, outputPanel/state, statusPanel/state, encoderPanel/state, metadataState, metadataSaveState) wrapped in framework reactivity (signals/runes/refs) but logic is preserved. |
| **Preserve as-is** (pure logic/types) | 1,400 | 29% | Domain logic (`statusPanel/domain/*`), services (`statusPanel/services/*`), formatting, validation, path building, types -- untouched. |

### Tauri IPC Surface (Compact and Clean)

**19 IPC touchpoints**, all through a single `bridge.ts` abstraction:
- **10 commands** (`invoke` calls): `analyze_audio_files`, `process_audiobook_files_v2`, `cancel_processing`, `save_metadata_to_file`, `read_audio_metadata`, `load_cover_art_file`, `load_cover_art_from_url`, `search_online_metadata`, `set_max_concurrent_jobs`, `list_available_encoders`
- **5 event listeners**: `tauri://drag-drop`, `tauri://drag-enter`, `tauri://drag-leave`, `processing-progress`, `processing-queue`
- **3 dialog invocations**: file picker, directory picker, image picker
- **1 external open**: open file in system player

In a framework migration, these would be wrapped in:
- **React**: custom hooks (`useInvoke`, `useListen`) or TanStack Query
- **Svelte**: stores or `$effect` blocks
- **Solid**: `createResource` / `createEffect`
- **Vue**: composables (`useInvoke`, `useListen`)

The `bridge.ts` abstraction layer already provides a clean wrapping point -- all Tauri APIs go through this single module, which simplifies the adaptation.

### Cross-Module State Dependencies

**Most depended-upon modules** (imported by the most other modules):
1. `fileList/state.ts` -- imported by 8+ modules (highest coupling)
2. `metadataState.ts` -- imported by 6+ modules
3. `coverArt.ts` -- imported by 5+ modules
4. `outputPanel` -- imported by 5+ modules
5. `metadataForm.ts` -- imported by 4+ modules

**Highest fan-out modules** (import from the most other modules):
1. `statusPanel/processing.ts` -- 11 import sources
2. `fileList/actions.ts` -- 10 import sources
3. `metadataLookup.ts` -- 9 import sources
4. `main.ts` -- 9 import sources (orchestrator)

In a framework migration, heavily-shared state like `fileList/state.ts` would need to be lifted into a reactive store (Zustand, Svelte stores, Solid signals, Pinia, etc.) to avoid prop-drilling or context complexity.

### Static HTML Size

**`index.html`: 961 lines**

In a component framework, virtually all 961 lines would move into component templates. The `index.html` would shrink to a mount point (`<div id="app">`) plus the `<head>` section (~15 lines).

---

## Summary Table

| Factor | Svelte 5 | SolidJS | Vue 4 | React 19 |
|--------|---------|---------|-------|----------|
| **Migration cost from vanilla TS** | **Lowest** | Moderate | Moderate | Highest |
| **Bundle overhead** | **~28 KB** | ~30 KB | ~58 KB | ~72 KB |
| **Runtime model match** | **Compiles to what we already write** | Fine-grained signals | Virtual DOM (optimized) | Virtual DOM |
| **DX for solo dev** | Excellent | Good (learning curve) | Excellent | Good (boilerplate) |
| **Ecosystem maturity** | Good (smaller) | Developing (2.0 pending) | Strong | Massive |
| **Tailwind v4 integration** | **Native** | Good | Good | Good (`className`) |
| **Co-location pattern** | **Best** (3-section SFC) | JSX only | Good (SFC) | JSX only |
| **Stability risk** | Low (5.x stable) | **Medium (2.0 coming)** | Low | Low |
| **State pattern mapping** | `state.ts` → `$state` runes (1:1) | `state.ts` → `createSignal` | `state.ts` → `ref()`/`reactive()` | `state.ts` → `useState`/`useEffect` |

---

## Design Governance Requirements (Opus + Engineering)

Before selecting and rolling out a framework, define these as non-negotiable guardrails:

1. **Single token source of truth**: spacing/color/typography tokens must be centralized and versioned
2. **Component ownership of UX intent**: each UI concern should own markup + behavior + style contract in one place
3. **Variant discipline**: reusable components expose explicit variants/states, not one-off utility chains
4. **Visual stability gates**: migration must include baseline screenshots for core flows (import, metadata edit, process/queue, cover art)
5. **Contract stability**: Rust/TS IPC (`bridge.ts`, command names, event payloads) remains stable during frontend migration

These guardrails are what convert a framework migration from "rewrite churn" into long-term maintainability leverage.

---

## Recommendation

**Svelte 5** remains the strongest baseline and should be paired with an Opus-informed design governance track:

1. **Compiles to the same imperative DOM code we're writing by hand** -- the runtime model is a perfect match
2. **Lowest migration cost** -- our `state.ts` → `$state` runes is nearly 1:1, our existing decomposition (state/dom/events/logic) maps cleanly to Svelte's architecture
3. **Smallest bundle** -- ~28 KB overhead (compiles away the framework)
4. **Best co-location pattern** -- `.svelte` files with `<script>` + markup + `<style>` in one file
5. **Native Tailwind v4 integration** -- works out of the box with Vite plugin
6. **Stable and production-ready** -- Svelte 5 + Runes is stable as of late 2024
7. **Design-system friendly for Opus workflows** -- single-file components + scoped styles + clear token mapping reduce drift and improve reviewability

**The main trade-off** is the smaller ecosystem compared to React/Vue. For a solo desktop app that isn't pulling in many third-party UI components, this is a manageable trade-off. Our domain logic, services, and validation code (~29% of the frontend) would survive untouched. The migration is about eliminating boilerplate and improving DX, not fixing structural problems.

**SolidJS** is the performance-first alternative if status panel rendering becomes a bottleneck, but the pending 2.0 migration and the steeper learning curve make it a riskier bet for a project in ship-it mode.

**Vue 4** is a solid middle ground with a strong ecosystem and incremental adoption story, but the bundle is larger and the runtime model (virtual DOM) is further from our current imperative patterns.

**React 19** has the highest migration cost and the least architectural alignment. Its value props (ecosystem for hiring, SSR, community for large teams) don't serve a solo desktop app.

---

---

## Full-Stack Type Safety: The Contract Layer

### The Problem

Right now, the Rust/TypeScript contract is **partially type-safe** but **manually maintained**:

**Current state**:
- Command name parity check only (via `scripts/ensure-contract.sh`)
- Stringly-typed `invoke()` calls in `bridge.ts`: `invoke<T>(commandName: string, args?: unknown)`
- Event payloads typed in TS (`src/types/events.ts`) but manually kept in sync with Rust
- No compile-time guarantee that TS types match Rust signatures
- Refactoring a Rust command signature requires manual TS type updates in multiple places

**Acknowledged gap** (from `src-tauri/AGENTS.md:41`):
> "until typesafe codegen is adopted"

This is a known technical debt item. The question is: what's the best path to resolve it?

### The Goal: One Source of Truth

**Full-stack type safety** means:
- **Rust command/event definitions are the single source of truth**
- **TypeScript types are auto-generated** from Rust signatures
- **Compile-time errors** if TS calls a Rust command with wrong args or wrong return type
- **Zero manual sync** between `src-tauri/src/commands/*.rs` and `src/types/*.ts`

### Why This Matters (Tri-Order Impact)

**Immediate (UX/DX)**:
- Fewer runtime IPC bugs (wrong arg types, missing fields, typos in command names)
- Safer refactors (change a Rust signature → TS compile error shows all affected call sites)
- Faster feature work (no manual type sync, no `ensure-contract.sh` detective work)

**Architectural Ripple**:
- Low-medium. Changes live in the contract layer (`bridge.ts`, `src/types/*`, Rust command annotations)
- Backend audio pipeline untouched
- Frontend UI modules minimally affected (just import updated types)

**Long-term Maintainability/Performance**:
- **Big maintainability win**: refactoring becomes safe, onboarding new AI collaborators is easier (types are the docs)
- **Near-zero runtime perf cost**: types are compile-time, erased at runtime
- **Main cost**: toolchain coupling (version alignment, build script maintenance, ecosystem churn risk)

### Options Analysis

#### Option 1: `tauri-specta` + `specta` (Code Generation)

**What it is**:
- [specta](https://github.com/specta-rs/specta): Rust library for exporting type definitions
- [tauri-specta](https://github.com/specta-rs/tauri-specta): Tauri integration for specta (generates TS bindings from Rust commands/events)

**How it works**:
1. Annotate Rust commands with `#[specta::specta]` macro
2. Add a build script that collects all annotated commands/events
3. Generate a `.ts` file with typed wrappers for all commands
4. Use the generated client in `bridge.ts` instead of raw `invoke()`

**Example**:
```rust
// src-tauri/src/commands/audio.rs
#[tauri::command]
#[specta::specta]  // Add this annotation
async fn analyze_audio_files(
  paths: Vec<String>,
) -> Result<Vec<AudioFileAnalysis>, AppError> {
  // ... existing implementation
}
```

Generated TS (simplified):
```ts
// src/bindings.ts (auto-generated)
export const commands = {
  analyzeAudioFiles: (paths: string[]): Promise<AudioFileAnalysis[]> => {
    return invoke('analyze_audio_files', { paths });
  },
  // ... all other commands
};
```

Usage in `bridge.ts`:
```ts
import { commands } from './bindings';

export const bridge = {
  invoke: commands,  // Now fully typed
  // ... rest of bridge
};
```

**Pros**:
- **Low migration friction**: keeps existing command names/signatures, just adds type safety layer
- **Incremental adoption**: can annotate commands one at a time (specta supports partial migration)
- **Active ecosystem**: specta is used in production by multiple Tauri apps, receives regular updates
- **Lightweight**: just adds a build script + macro annotations, no major architectural changes
- **Tauri-first**: designed specifically for Tauri's invoke/listen patterns
- **Future-proof for cross-platform**: works on Windows/Linux/macOS identically (TS generation is platform-agnostic)

**Cons**:
- **Ecosystem maturity risk**: specta is younger than Tauri itself (first stable release 2023), could have breaking changes
- **Build toolchain coupling**: adds a build step (Rust → generate TS → bundle). If specta breaks, dev workflow stops.
- **Version alignment**: must keep `tauri-specta` version aligned with `tauri` version (documented compatibility matrix exists but adds maintenance)
- **Macro magic**: `#[specta::specta]` is compile-time magic, harder to debug when it fails
- **No WIT/IDL**: not based on a standard interface definition language (see Option 3)

**Migration effort**: ~1-2 days
- Add dependencies (`specta`, `tauri-specta`)
- Write build script to collect/export commands
- Annotate 10 commands + 2 event types in Rust
- Refactor `bridge.ts` to use generated client
- Update 19 IPC call sites to use typed imports
- Test, verify, retire `ensure-contract.sh`

**Rollback**: Easy. Remove deps, delete generated file, revert `bridge.ts`. All Rust code unchanged (annotations are no-ops if specta is removed).

**Cross-platform note**: TS generation is platform-agnostic. The same generated types work on Windows/Linux/macOS. Only the Rust build target changes, not the contract layer.

---

#### Option 2: `TauRPC` (RPC Framework)

**What it is**:
- [TauRPC](https://github.com/MatsDK/TauRPC): End-to-end type-safe RPC framework for Tauri, inspired by tRPC (React ecosystem)

**How it works**:
1. Define a "router" in Rust with typed procedures
2. TauRPC generates both Rust handlers and TS client
3. Replace raw `invoke()` calls with typed RPC client

**Example**:
```rust
// Define a router (new pattern, replaces #[tauri::command])
router! {
  analyze_audio_files(paths: Vec<String>) -> Result<Vec<AudioFileAnalysis>, AppError> {
    // ... implementation
  }
}
```

Generated TS:
```ts
const client = createTauRPCClient();
await client.analyzeAudioFiles(['path1.m4a']);  // Fully typed
```

**Pros**:
- **Very strong type safety**: strictest option, closest to "single source of truth" (the router definition)
- **Excellent DX**: TypeScript autocomplete for all procedures, args, return types
- **Rust-first**: procedure definitions live in Rust, TS is 100% generated

**Cons**:
- **High migration cost**: requires rewriting all 10 commands as router procedures (different API shape than `#[tauri::command]`)
- **Opinionated API changes**: changes how you call commands in TS (RPC client instead of `invoke()`)
- **Smaller ecosystem**: TauRPC is newer and less widely adopted than specta
- **More invasive**: touches more of the Rust codebase (command handler signatures change)
- **Unclear cross-platform story**: docs don't explicitly cover Windows/Linux, but should work (it's just codegen)

**Migration effort**: ~3-4 days
- Rewrite 10 commands as router procedures
- Replace `invoke()` calls with RPC client calls
- Test, verify
- Higher risk (more invasive changes)

**Rollback**: Moderate. Requires reverting both Rust command signatures AND all TS call sites.

**Verdict for this repo**: **Too invasive for ship-it mode**. The stronger guarantees don't justify the migration cost when we're trying to minimize churn before launch.

---

#### Option 3: `tauri-bindgen` (WIT/IDL-Based)

**What it is**:
- [tauri-bindgen](https://github.com/tauri-apps/tauri-bindgen): Official Tauri experiment using WebAssembly Interface Types (WIT) as a contract IDL
- **Status**: Experimental, not production-ready (per Tauri team, Feb 2026)

**How it works**:
1. Define contracts in a `.wit` file (interface definition language)
2. `tauri-bindgen` generates both Rust stubs and TS types from the WIT file
3. WIT file becomes the single source of truth (neither Rust nor TS owns it)

**Example WIT**:
```wit
interface audio {
  analyze-audio-files: func(paths: list<string>) -> result<list<audio-file-analysis>, app-error>
}
```

**Pros**:
- **Strongest "single source of truth"**: WIT file is language-agnostic, explicit, version-controllable
- **Standard-based**: WIT is a WebAssembly standard (WASI), not a Tauri-specific invention
- **Official Tauri support**: built by Tauri core team (future-proof if it becomes the official way)
- **Best for polyglot**: if we ever wanted to add a Python backend or similar, WIT would support it

**Cons**:
- **Not production-ready**: still experimental as of Feb 2026, API is unstable
- **Higher complexity**: requires learning WIT syntax, maintaining a separate IDL file
- **More moving parts**: three sources (WIT + Rust + TS) instead of two
- **Unclear timeline**: no clear "stable" release date from Tauri team
- **Higher migration cost**: ~4-5 days (write WIT contracts + adapt Rust + adapt TS)

**Rollback**: Hard. WIT becomes a hard dependency, reverting requires manually reconstructing Rust signatures.

**Verdict for this repo**: **Wait for stable release**. The standard-based approach is appealing, but experimental status is a non-starter for ship-it mode. Revisit post-launch if Tauri promotes it to stable.

---

### Recommended Option: `tauri-specta`

**Why**:
1. **Lowest migration friction**: keeps existing Rust command signatures, just adds type annotations
2. **Incremental adoption**: can annotate one command at a time, test, rollback if needed
3. **Production-ready**: actively used in real Tauri apps (specta has 1.5k+ GitHub stars, regular releases)
4. **Tauri-first**: designed for Tauri's invoke/listen patterns (not a generic RPC framework)
5. **Easy rollback**: annotations are no-ops if specta is removed, minimal Rust code changes
6. **Cross-platform proven**: TS generation is platform-agnostic, works on macOS/Windows/Linux identically

**Risk assessment**:
- **Ecosystem maturity**: specta is younger (2023) than Tauri (2019), but has proven stability in production
- **Version churn**: requires tracking `tauri-specta` ↔ `tauri` compatibility, but documented matrix exists
- **Build coupling**: adds a build step, but isolated (if it fails, easy to debug/fix)

**Mitigation for risks**:
- Pin `tauri-specta` and `specta` versions in `Cargo.toml` (don't auto-update)
- Document the version alignment in `CLAUDE.md` or `src-tauri/AGENTS.md`
- Keep `ensure-contract.sh` around (disabled) for 1-2 releases as a fallback sanity check

**Not recommended**:
- **TauRPC**: Too invasive, changes too much Rust code for marginal type-safety improvement
- **tauri-bindgen**: Too experimental, wait for stable release

**Manual type sync (status quo)**: Still an option if you want to defer this entirely, but it's technical debt that will grow as the app adds more commands/events. Better to tackle now before Windows/Linux ports add complexity.

---

### Cross-Platform Considerations (Windows/Linux Future)

**Current state**: macOS-only, Apple Silicon primary target

**Future goal** (inferred): Windows and Linux versions

**How type safety helps**:
- **Platform-agnostic contracts**: TS types generated from Rust work identically on Windows/Linux/macOS
- **Safer platform-specific code**: if you add platform-specific Rust logic (e.g., Windows-only file dialogs), type system enforces consistent TS interface
- **Easier CI/CD**: type checks run on all platforms in CI, catch platform-specific contract drift

**How type safety could hurt**:
- **Build toolchain complexity**: Windows Rust toolchain + Node/Bun + specta codegen = more moving parts
- **Version skew**: if Windows CI uses a different Rust/Node version, specta codegen might differ (rare but possible)

**Mitigation**:
- Use `rust-toolchain.toml` to pin Rust version across platforms (already a best practice)
- Use `.nvmrc` or similar to pin Node/Bun version
- Run codegen in CI on all platforms, fail if generated TS differs (ensures consistency)

**Verdict**: Type safety is a **net win for cross-platform**. The contract enforcement is more valuable when you have 3 build targets instead of 1.

---

## Decision Matrix: Type Safety + Framework Migration

These two decisions are **orthogonal** but **sequenced**:

| Decision | Scope | Risk | Value | When |
|----------|-------|------|-------|------|
| **Full-stack type safety** (tauri-specta) | Contract layer (`bridge.ts`, Rust commands, types) | Low | Immediate (safer refactors, fewer IPC bugs) | **Phase 1** (do first) |
| **Framework migration** (Svelte) | Presentation layer (`src/ui/*`, 48% of frontend) | Medium-High | Deferred (co-location, reactivity pay off over time) | **Phase 3-4** (do later) |

**Why this sequence**:

### 1. Risk De-escalation
- **tauri-specta**: Low-risk, mechanical change. Contract layer only.
- **Svelte**: High-risk, major rewrite. Touches half the frontend.

**Principle**: Tackle the safe change first, so the risky change has one less variable.

### 2. Stability During Migration
With tauri-specta in place first:
- Type-safe IPC is a **stable foundation** while rewriting UI components
- If a Svelte component breaks, you know it's a component bug, not an IPC contract drift
- Generated types serve as a **safety net** during the framework rewrite

Without tauri-specta first:
- Simultaneous framework learning curve AND manual contract sync
- Any IPC bug could be "is this a Svelte issue or a contract drift?"

### 3. Incremental Value
- **tauri-specta**: Immediate value. Better DX today, no UI disruption.
- **Svelte**: Deferred value. Benefits emerge as migration completes.

### 4. Design Governance Alignment
The design governance requirements (lines 368-378) include:
> 5. **Contract stability**: Rust/TS IPC (`bridge.ts`, command names, event payloads) remains stable during frontend migration

**With tauri-specta first**, you've already locked down contract stability (#5) before the framework migration starts. That's one less guardrail to manually enforce.

---

## Recommended Phased Approach

### Phase 1: Full-Stack Type Safety (tauri-specta)
**Outcome**: Type-safe IPC, retire `ensure-contract.sh`, generated bindings in `bridge.ts`

**Estimated scope**: 1-2 days work
1. Add `tauri-specta` + `specta` dependencies to `src-tauri/Cargo.toml`
2. Create build script (`src-tauri/build.rs` or separate script) to collect annotated commands and generate TS
3. Annotate 10 Rust commands with `#[specta::specta]` in `src-tauri/src/commands/audio.rs` and `src-tauri/src/commands/metadata.rs`
4. Annotate 2 event types (`ProcessingProgress`, `ProcessingQueue`) in `src-tauri/src/audio/progress/mod.rs`
5. Refactor `src/lib/bridge.ts` to use generated client instead of raw `invoke<T>(commandName: string, args?: unknown)`
6. Update 19 IPC call sites across `src/ui/*` to import and use typed command wrappers
7. Run `scripts/standard-checks.sh` to verify tests still pass
8. Disable (but don't delete) `ensure-contract.sh` name parity checks (keep as fallback for 1 release)
9. Document version alignment in `CLAUDE.md` or `src-tauri/AGENTS.md`

**Risk**: Low. Contract layer only, no UI changes. Easy rollback (remove deps, revert `bridge.ts`).

**Dependencies**: None. Can start immediately.

---

### Phase 2: Design + Architecture Checkpoint (with Opus)
**Outcome**: Token source-of-truth, component variant strategy, baseline screenshots

**Estimated scope**: 2-3 days work
1. Define spacing/color/typography tokens (migrate from CSS custom properties in `src/styles.css` to centralized system)
2. Decide: Tailwind theme config vs. Svelte CSS vars vs. hybrid
3. Establish component variant patterns (e.g., `<Button variant="primary" | "secondary" | "danger">`)
4. Capture baseline screenshots for core flows:
   - File import (drag-drop, file picker, file list rendering)
   - Metadata editing (form, cover art, online lookup)
   - Processing (progress bar, job list, queue status, cancel)
   - Output (directory selection, ABS structure toggle, path preview)
5. Document design system in `docs/design-system.md` (or similar):
   - Token definitions and usage rules
   - Component variant catalog
   - Spacing/color/typography scales
   - Accessibility requirements (keyboard nav, focus states, etc.)
6. Run perf baselines (`bun run perf:all --compare-baseline`) to establish pre-migration benchmark

**Risk**: Low. Planning and documentation only, no code changes yet.

**Dependencies**: Phase 1 complete (so contract layer is stable).

---

### Phase 3: Framework Migration Spike (Svelte)
**Outcome**: One small panel converted to Svelte as proof-of-concept

**Estimated scope**: 2-3 days work
1. Add Svelte dependencies: `svelte@5.x`, `@sveltejs/vite-plugin-svelte`
2. Update `vite.config.ts` to include Svelte plugin
3. Choose spike target: **JobControls** (149 LOC, simple, low coupling) or **TagPreview** (113 LOC, pure data-driven)
4. Convert chosen panel to `.svelte` component:
   - Move state from `let` variables to `$state` runes
   - Move markup from `index.html` section to Svelte template
   - Move event handlers to `on:click` / `on:change` directives
   - Test IPC calls (should work seamlessly with tauri-specta types from Phase 1)
5. Test Tailwind + scoped styles pattern (use both inline utilities and `<style>` block)
6. Measure bundle size impact (compare `dist/` size before/after)
7. Verify baseline screenshot still matches (visual regression check)
8. Document lessons learned: what worked, what didn't, gotchas encountered

**Risk**: Low. Single component, easy to revert (delete `.svelte` file, restore old TS module).

**Dependencies**: Phases 1-2 complete (contract stable, design system defined).

---

### Phase 4: Phased Framework Migration (if spike succeeds)
**Outcome**: Full frontend migrated to Svelte, component by component

**Estimated scope**: 3-4 weeks work (staggered, one panel at a time)

**Migration sequence** (low-risk → high-complexity):
1. **JobControls** (149 LOC) -- already done in spike, just formalize
2. **TagPreview** (113 LOC) -- pure data display, no complex state
3. **OutputPanel** (560 LOC) -- moderate complexity, well-decomposed
4. **EncoderPanel** (470 LOC) -- moderate complexity, well-decomposed
5. **CoverArt** (436 LOC) -- drag/drop, file loading, moderate state
6. **MetadataForm** (445 LOC) -- form state, dirty tracking, validation
7. **FileImport** (178 LOC) -- drag/drop, file dialog, moderate coupling
8. **MetadataLookup** (582 LOC) -- modal, online search, complex DOM building
9. **FileList** (620 LOC, 6 files) -- high complexity, multi-select, reordering, metadata panel
10. **StatusPanel** (1,300 LOC, 12 files) -- **highest complexity**, progress rendering, job orchestration, queue state

**Process for each panel**:
1. Create `.svelte` component in `src/ui/{panel-name}.svelte` or `src/ui/{panel-name}/Component.svelte`
2. Migrate state: `let foo` → `let foo = $state(...)`
3. Migrate markup: copy from `index.html`, convert to Svelte template syntax
4. Migrate events: `addEventListener` → `on:click` / `on:change` / etc.
5. Migrate IPC: import typed commands from generated bindings (already done in Phase 1)
6. Keep old TS module alongside new component until tests pass (dual-render capability)
7. Run visual regression test (compare screenshot to baseline)
8. Update tests to use Svelte component if needed
9. Delete old TS module once new component is proven
10. Update `index.html` to remove migrated section, replace with `<Component />` mount point
11. Commit at panel boundary (logical unit of work)

**Guardrails**:
- Maintain type safety via tauri-specta bindings (from Phase 1)
- Follow design system rules (from Phase 2)
- Visual regression test after each panel
- Can pause/ship at any panel boundary if timeline pressure hits

**Risk**: Medium-High. Large refactor, but phased to limit blast radius. Each panel migration is reversible.

**Dependencies**: Phase 3 spike must succeed and validate assumptions.

---

### Phase 5 (Optional): Post-Migration Cleanup
**Outcome**: Remove hybrid cruft, consolidate design tokens, optimize bundle

**Estimated scope**: 1-2 days work
1. Shrink `src/styles.css` (move component-specific styles into `.svelte` files, keep only global/token styles)
2. Consolidate design tokens (decide final token source: Tailwind config, CSS vars, or Svelte stores)
3. Audit bundle size (run `bun run build`, analyze `dist/` size, check for duplication)
4. Remove old TS module scaffolding if any remains
5. Update `CLAUDE.md` / `src/AGENTS.md` with new Svelte conventions
6. Run full perf suite (`bun run perf:all --compare-baseline`) to measure before/after impact

**Risk**: Low. Cleanup phase, no new features.

**Dependencies**: Phase 4 complete (full migration done).

---

## Alternative: Defer Both (Status Quo)

**When to choose this**:
- You're in "ship and learn" mode -- need to get Windows/Linux ports done first, worry about type safety + framework later
- Current manual contract sync (`ensure-contract.sh`) is working fine, not causing bugs
- Current vanilla TS UI is fast enough, co-location isn't a pain point yet

**Trade-offs**:
- **Technical debt grows**: more commands = more manual sync points, more risk of drift
- **Windows/Linux ports harder**: manual contract sync across 3 platforms is more error-prone
- **Framework migration gets harder**: the longer you wait, the more UI code exists, the bigger the rewrite

**When to revisit**:
- After shipping Windows/Linux ports (if type safety becomes a pain point)
- After adding 5+ new commands (if manual sync starts causing bugs)
- After 6 months (if co-location pain becomes real DX friction)

---

## Bottom Line for a Senior Engineer Review

**The case for tauri-specta (Phase 1)**:
- **Low-risk, high-value**: 1-2 days work, immediate DX improvement, easy rollback
- **Future-proof**: helps with Windows/Linux ports (contract enforcement across platforms)
- **Enables safer Svelte migration**: type-safe IPC is a stable foundation during UI rewrite
- **Production-proven**: actively used in real Tauri apps, not experimental
- **Incremental**: can adopt one command at a time, test, rollback if issues

**The case against tauri-specta**:
- **Ecosystem maturity risk**: younger than Tauri (2023 vs 2019), could have breaking changes
- **Build coupling**: adds a build step (if specta breaks, dev workflow stops)
- **Version alignment**: must track `tauri-specta` ↔ `tauri` compatibility matrix
- **Not a standard**: specta is Rust-specific, not based on WIT/IDL (unlike tauri-bindgen)

**The case for Svelte migration (Phases 2-4)**:
- **Lowest migration cost** of any framework (compiles to imperative DOM, matches current architecture)
- **Best co-location story**: `<script>` + markup + `<style>` in one file
- **Design-system friendly**: single-file components + scoped styles reduce drift
- **Production-ready**: Svelte 5 + Runes stable as of late 2024

**The case against Svelte migration**:
- **High migration cost**: ~48% of frontend needs rewrite (2,300 LOC of DOM/templates/events)
- **Smaller ecosystem**: fewer pre-built components than React/Vue (less relevant for desktop app)
- **Deferred value**: benefits emerge over time, not immediate

**Recommended decision**:
1. **Do Phase 1 (tauri-specta) now**: low-risk, immediate value, helps with everything else
2. **Do Phase 2 (design checkpoint) next**: locks down design system before code migration
3. **Spike Phase 3 (Svelte POC)**: validate assumptions, measure real migration cost
4. **Decide on Phase 4 (full migration) after spike**: you'll have real data, not just theory

**Alternative decision** (defer both):
- Valid if ship-it mode requires focus on Windows/Linux ports first
- Revisit after cross-platform launch, when type safety + co-location ROI is clearer

---

## Sources (Type Safety)

### tauri-specta + specta
- [tauri-specta GitHub](https://github.com/specta-rs/tauri-specta)
- [specta GitHub](https://github.com/specta-rs/specta)
- [specta Documentation](https://docs.rs/specta/latest/specta/)

### TauRPC
- [TauRPC GitHub](https://github.com/MatsDK/TauRPC)

### tauri-bindgen
- [tauri-bindgen GitHub](https://github.com/tauri-apps/tauri-bindgen)
- [WebAssembly Interface Types (WIT)](https://component-model.bytecodealliance.org/design/wit.html)

---

## Next Steps (If/When Ready to Act)

## Sources

### Tauri Official Docs
- [Tauri Frontend Configuration](https://v2.tauri.app/start/frontend/)
- [Tauri 2.0 Stable Release](https://v2.tauri.app/blog/tauri-20/)
- [Create a Project](https://v2.tauri.app/start/create-project/)

### Framework Comparisons
- [CrabNebula: Best UI Libraries for Tauri](https://crabnebula.dev/blog/the-best-ui-libraries-for-cross-platform-apps-with-tauri/)
- [FrontendTools: React vs Vue vs Svelte vs SolidJS 2025-2026](https://www.frontendtools.tech/blog/best-frontend-frameworks-2025-comparison)
- [js-framework-benchmark](https://github.com/krausest/js-framework-benchmark)

### Svelte 5
- [Svelte 5 2025 Review (Scalable Path)](https://www.scalablepath.com/javascript/svelte-5-review)
- [Svelte & SvelteKit Summer 2025 Recap](https://blog.openreplay.com/svelte-sveltekit-updates-summer-2025-recap/)
- [Svelte in 2025: Production Ready (Codify)](https://codifysol.com/svelte-in-2025-is-it-ready-for-production/)
- [What's new in Svelte: December 2025](https://svelte.dev/blog/whats-new-in-svelte-december-2025)
- [Svelte 5 Migration Guide](https://svelte.dev/docs/svelte/v5-migration-guide)
- [Adding TypeScript to Svelte (LogRocket)](https://blog.logrocket.com/adding-typescript-existing-svelte-project/)
- [Svelte Tailwind CSS v4 Discussion](https://github.com/sveltejs/svelte/discussions/14668)
- [Tailwind CSS SvelteKit Guide](https://tailwindcss.com/docs/guides/sveltekit)

### SolidJS
- [SolidJS 2.0 Roadmap Discussion](https://github.com/solidjs/solid/discussions/2425)
- [SolidJS Releases](https://github.com/solidjs/solid/releases)
- [SolidJS in 2025 (JS Doctor)](https://www.javascriptdoctor.blog/2025/05/solidjs-in-2025-build-high-performance.html)
- [SolidJS Comparison Guide](https://www.solidjs.com/guides/comparison)
- [Tailwind CSS SolidJS Guide](https://tailwindcss.com/docs/guides/solidjs)
- [SolidJS Styling Docs](https://docs.solidjs.com/guides/styling-components/tailwind)
