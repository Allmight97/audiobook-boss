import { bridge } from "../lib/bridge";
import type { JobType } from "../types/audio";
import { mount, unmount } from "svelte";
import JobControlsIsland from "./jobControls/JobControlsIsland.svelte";

const JOB_CONTROLS_ROOT_ID = "job-controls-root";
const MERGE_TOGGLE_ID = "merge-mode-toggle";
const MAX_CONCURRENT_SELECT_ID = "max-concurrent-select";
const MAX_CONCURRENT_EFFECTIVE_ID = "max-concurrent-effective";
const MAX_CONCURRENT_STORAGE_KEY = "abb:maxConcurrentJobs";

let effectiveMaxConcurrent: number | null = null;
let maxConcurrentSelection: string = "auto";
let mountedControlsRoot: HTMLElement | null = null;
let mountedControlsIsland: Parameters<typeof unmount>[0] | null = null;

export function initJobControls(): void {
  mountJobControlsIsland();
  initializeMaxConcurrentControl();
  initializeJobTypeControl();
}

function mountJobControlsIsland(): void {
  const controlsRoot = document.getElementById(JOB_CONTROLS_ROOT_ID);
  if (!controlsRoot) return;

  if (
    mountedControlsIsland &&
    mountedControlsRoot === controlsRoot &&
    controlsRoot.childElementCount > 0
  ) {
    return;
  }

  if (mountedControlsIsland) {
    void unmount(mountedControlsIsland);
    mountedControlsIsland = null;
  }

  mountedControlsIsland = mount(JobControlsIsland, { target: controlsRoot });
  mountedControlsRoot = controlsRoot;
}

function initializeJobTypeControl(): void {
  const toggle = document.getElementById(MERGE_TOGGLE_ID) as HTMLInputElement | null;
  if (!toggle) return;

  // Potential: Restore saved preference if we want to persist it
  toggle.addEventListener("change", () => {
    document.dispatchEvent(new Event("abb:job-type-changed"));
  });
}

function initializeMaxConcurrentControl(): void {
  const select = document.getElementById(MAX_CONCURRENT_SELECT_ID) as HTMLSelectElement | null;
  if (!select) return;

  // Restore saved preference
  const saved = readMaxConcurrentPreference();
  select.value = saved;
  maxConcurrentSelection = saved;

  select.addEventListener("change", () => {
    const value = select.value;
    maxConcurrentSelection = value;
    writeMaxConcurrentPreference(value);
    void pushMaxConcurrentToBackend(value);
  });

  // Push initial selection
  void pushMaxConcurrentToBackend(saved);
}

// Read current value
export function getJobType(): JobType {
  const toggle = document.getElementById(MERGE_TOGGLE_ID) as HTMLInputElement | null;
  if (!toggle) return "batch";
  return toggle.checked ? "merge" : "batch";
}

export function getMaxConcurrentStatus(): {
  effective: number | null;
  selection: string;
} {
  return {
    effective: effectiveMaxConcurrent,
    selection: maxConcurrentSelection,
  };
}

export function setJobControlsEnabled(enabled: boolean): void {
  const mergeToggle = document.getElementById(MERGE_TOGGLE_ID) as HTMLInputElement | null;
  const maxConcurrentSelect = document.getElementById(
    MAX_CONCURRENT_SELECT_ID
  ) as HTMLSelectElement | null;

  if (mergeToggle) {
    mergeToggle.disabled = !enabled;
    mergeToggle.style.opacity = enabled ? "1" : "0.5";
  }

  if (maxConcurrentSelect) {
    maxConcurrentSelect.disabled = !enabled;
    maxConcurrentSelect.style.opacity = enabled ? "1" : "0.5";
  }
}

function updateMaxConcurrentIndicator(): void {
  const indicator = document.getElementById(MAX_CONCURRENT_EFFECTIVE_ID) as HTMLElement | null;
  if (!indicator) return;

  if (effectiveMaxConcurrent === null) {
    indicator.textContent = "";
    return;
  }

  if (maxConcurrentSelection === "auto") {
    indicator.textContent = `Auto → ${effectiveMaxConcurrent}`;
  } else {
    indicator.textContent = `Max ${effectiveMaxConcurrent}`;
  }
}

function readMaxConcurrentPreference(): string {
  // FALLBACK[FB-004]: trigger=localStorage unavailable/blocked (privacy mode, restricted contexts)
  // observe=console.warn markers on read/write/parse fallback paths
  // sunset=2026-04-30 issue=#199
  if (typeof localStorage === "undefined" || typeof localStorage.getItem !== "function") {
    console.warn("FALLBACK[FB-004] localStorage.getItem unavailable; using auto max concurrency");
    return "auto";
  }
  try {
    return localStorage.getItem(MAX_CONCURRENT_STORAGE_KEY) ?? "auto";
  } catch (error) {
    console.warn("FALLBACK[FB-004] failed to read max concurrency preference; using auto", error);
    return "auto";
  }
}

function writeMaxConcurrentPreference(value: string): void {
  if (typeof localStorage === "undefined" || typeof localStorage.setItem !== "function") {
    return;
  }
  try {
    localStorage.setItem(MAX_CONCURRENT_STORAGE_KEY, value);
  } catch (error) {
    // localStorage may be unavailable in private browsing; non-critical
    console.warn("FALLBACK[FB-004] failed to persist max concurrency preference", error);
  }
}

async function pushMaxConcurrentToBackend(value: string): Promise<void> {
  try {
    let effective: number;
    if (value === "auto") {
      effective = await bridge.setMaxConcurrentJobs(null);
    } else {
      const parsed = parseInt(value, 10);
      if (!Number.isFinite(parsed)) {
        console.warn("FALLBACK[FB-004] invalid max concurrency selection ignored:", value);
        return;
      }
      effective = await bridge.setMaxConcurrentJobs(parsed);
    }
    if (!Number.isFinite(effective)) {
      return;
    }
    effectiveMaxConcurrent = effective;
    updateMaxConcurrentIndicator();
    document.dispatchEvent(
      new CustomEvent("abb:max-concurrent-updated", {
        detail: { effective, selection: maxConcurrentSelection },
      })
    );
  } catch (error) {
    console.warn("Failed to update max concurrency:", error);
  }
}
