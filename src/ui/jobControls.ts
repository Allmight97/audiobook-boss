import { bridge } from "../lib/bridge";
import type { JobType } from "../types/audio";

const JOB_TYPE_SELECT_ID = "job-type-select";
const MAX_CONCURRENT_SELECT_ID = "max-concurrent-select";
const MAX_CONCURRENT_STORAGE_KEY = "abb:maxConcurrentJobs";

export function initJobControls(): void {
    initializeMaxConcurrentControl();
    initializeJobTypeControl();
}

function initializeJobTypeControl(): void {
    const select = document.getElementById(JOB_TYPE_SELECT_ID) as HTMLSelectElement;
    if (!select) return;

    // Potential: Restore saved preference if we want to persist it
    select.addEventListener("change", () => {
        document.dispatchEvent(new Event("abb:job-type-changed"));
    });
}

function initializeMaxConcurrentControl(): void {
    const select = document.getElementById(
        MAX_CONCURRENT_SELECT_ID
    ) as HTMLSelectElement | null;
    if (!select) return;

    // Restore saved preference
    const saved = readMaxConcurrentPreference();
    select.value = saved;

    select.addEventListener("change", () => {
        const value = select.value;
        writeMaxConcurrentPreference(value);
        void pushMaxConcurrentToBackend(value);
    });

    // Push initial selection
    void pushMaxConcurrentToBackend(saved);
}

// Read current value
export function getJobType(): JobType {
    const select = document.getElementById(JOB_TYPE_SELECT_ID) as HTMLSelectElement;
    if (!select) return "merge";

    const val = select.value;
    if (val === "batch") return "batch";
    return "merge";
}

export function setJobControlsEnabled(enabled: boolean): void {
    const jobTypeSelect = document.getElementById(JOB_TYPE_SELECT_ID) as HTMLSelectElement;
    const maxConcurrentSelect = document.getElementById(MAX_CONCURRENT_SELECT_ID) as HTMLSelectElement;

    if (jobTypeSelect) {
        jobTypeSelect.disabled = !enabled;
        if (!enabled) jobTypeSelect.style.opacity = "0.5";
        else jobTypeSelect.style.opacity = "1";
    }

    if (maxConcurrentSelect) {
        maxConcurrentSelect.disabled = !enabled;
        if (!enabled) maxConcurrentSelect.style.opacity = "0.5";
        else maxConcurrentSelect.style.opacity = "1";
    }
}

function readMaxConcurrentPreference(): string {
    if (typeof localStorage === "undefined" || typeof localStorage.getItem !== "function") {
        return "auto";
    }
    try {
        return localStorage.getItem(MAX_CONCURRENT_STORAGE_KEY) ?? "auto";
    } catch (_e) {
        return "auto";
    }
}

function writeMaxConcurrentPreference(value: string): void {
    if (typeof localStorage === "undefined" || typeof localStorage.setItem !== "function") {
        return;
    }
    try {
        localStorage.setItem(MAX_CONCURRENT_STORAGE_KEY, value);
    } catch (_e) {
        // ignore
    }
}

async function pushMaxConcurrentToBackend(value: string): Promise<void> {
    try {
        if (value === "auto") {
            await bridge.invoke("set_max_concurrent_jobs", { max_concurrent: null });
        } else {
            const parsed = parseInt(value, 10);
            if (!Number.isFinite(parsed)) {
                return;
            }
            await bridge.invoke("set_max_concurrent_jobs", {
                max_concurrent: parsed,
            });
        }
    } catch (error) {
        console.warn("Failed to update max concurrency:", error);
    }
}
