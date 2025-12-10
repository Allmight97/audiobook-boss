// REFACTOR: Module exceeds 400 LOC (569). Consider splitting before adding new code.
/**
 * StatusPanel business logic and state management
 *
 * This module contains the core StatusPanel class with event handling,
 * processing coordination, and state management.
 */

import { bridge } from "../../lib/bridge";
import { ProcessingProgressEvent, EVENTS, STAGES } from "../../types/events";
import { currentFileList } from "../fileList";
import { getCurrentOutputConfig } from "../outputPanel";
import type { EncoderSettings, OutputConfig } from "../../types/audio";
import {
  defaultEncoderSettings,
  VALID_ENCODER_BITRATES,
} from "../../types/audio";
import { toBoundaryEncoderSettings } from "../../types/encoder";
import type { EncoderSettingsLike } from "../../types/encoder";
import { AudiobookMetadata } from "../../types/metadata";
import * as dom from "./dom";
import { getCurrentCoverArt } from "../coverArt";
import { getJobType, setJobControlsEnabled } from "../jobControls";

interface ProcessingStatus {
  stage:
  | "idle"
  | "analyzing"
  | "converting"
  | "writing"
  | "completed"
  | "cancelled"
  | "failed";
  percentage: number;
  message: string;
  currentFile?: string;
  etaSeconds?: number;
}

/** Per-job progress tracking for parallel batch processing */
interface JobProgress {
  jobId: string;
  stage: ProcessingStatus["stage"];
  percentage: number;
  message: string;
  lastUpdate: number;
}

/** Aggregate progress across all active jobs */
interface AggregateProgress {
  activeJobs: number;
  completedJobs: number;
  overallPercentage: number;
}

type WindowWithEncoderProvider = Window & {
  EncoderSettingsProvider?: () => EncoderSettingsLike;
};

// Derived from centralized VALID_ENCODER_BITRATES (audio.ts)
// Typed as Set<number> to allow membership check with any numeric bitrate
const SUPPORTED_ENCODER_BITRATES: Set<number> = new Set(VALID_ENCODER_BITRATES);
// const MAX_CONCURRENT_STORAGE_KEY = "abb:maxConcurrentJobs";

export class StatusPanel {
  private cancelUnlisten?: () => void;
  private isProcessing: boolean = false;
  private currentStatus: ProcessingStatus;
  private previewDuration: number = 30;
  /** Per-job progress tracking for parallel batch processing */
  private jobProgress: Map<string, JobProgress> = new Map();

  constructor() {
    this.currentStatus = {
      stage: "idle",
      percentage: 0,
      message: "Ready to process audiobook",
    };

    this.initializeElements();
    this.setupEventHandlers();
    // initialized in main.ts now: this.initializeMaxConcurrentControl();

    // Ensure event listeners are cleaned up if the window unloads
    window.addEventListener("beforeunload", () => {
      if (this.cancelUnlisten) {
        this.cancelUnlisten();
        this.cancelUnlisten = undefined;
      }
    });
  }

  private initializeElements(): void {
    const elements = dom.initializeElements();
    if (!elements) {
      console.error("StatusPanel: Required DOM elements not found");
      return;
    }

    // Set initial UI state
    this.updateUI();

    // Initialize art thumbnail to placeholder
    dom.resetArtThumbnail();
  }

  private setupEventHandlers(): void {
    const processButton = dom.getProcessButton();
    const previewButton = document.getElementById(
      "preview-button"
    ) as HTMLButtonElement | null;
    const previewDropdownToggle = document.getElementById(
      "preview-dropdown-toggle"
    ) as HTMLButtonElement | null;
    const previewDropdown = document.getElementById(
      "preview-dropdown"
    ) as HTMLDivElement | null;
    const advancedToggle = document.getElementById(
      "advanced-settings-toggle"
    ) as HTMLButtonElement | null;

    if (processButton) {
      processButton.addEventListener("click", () => this.startProcessing());
    }

    const cancelAllButton = dom.getCancelAllButton();
    if (cancelAllButton) {
      cancelAllButton.addEventListener("click", () => this.handleCancelAll());
    }

    if (previewButton) {
      previewButton.addEventListener("click", async () => {
        await this.startProcessing({ previewSeconds: this.previewDuration });
      });
    }

    // Preview duration dropdown
    if (previewDropdownToggle && previewDropdown) {
      previewDropdownToggle.addEventListener("click", (e) => {
        e.stopPropagation();
        previewDropdown.style.display =
          previewDropdown.style.display === "none" ? "block" : "none";
      });

      // Handle duration options
      previewDropdown.querySelectorAll(".split-option").forEach((opt) => {
        opt.addEventListener("click", () => {
          const duration = parseInt(
            (opt as HTMLElement).dataset.duration || "30",
            10
          );
          this.previewDuration = duration;
          previewDropdown.style.display = "none";
          // Optionally trigger preview with new duration
          this.startProcessing({ previewSeconds: duration });
        });
      });

      // Close dropdown on outside click
      document.addEventListener("click", () => {
        previewDropdown.style.display = "none";
      });
    }

    // Advanced settings accordion
    if (advancedToggle) {
      advancedToggle.addEventListener("click", () => {
        const panel = document.getElementById("advanced-settings-panel");
        const icon = document.getElementById("advanced-toggle-icon");
        if (panel) {
          panel.classList.toggle("open");
          if (icon)
            icon.textContent = panel.classList.contains("open") ? "▼" : "▶";
        }
      });
    }
  }

  // MaxConcurrent control moved to src/ui/jobControls.ts

  public async startProcessing(options?: {
    previewSeconds?: number;
  }): Promise<void> {
    try {
      console.log("StatusPanel: Starting processing...");
      console.log("Current file list:", currentFileList);

      // Validate inputs
      if (
        !currentFileList ||
        !currentFileList.files ||
        currentFileList.files.length === 0
      ) {
        console.log("StatusPanel: No files found");
        dom.showError("No audio files selected. Please add files to process.");
        return;
      }

      if (currentFileList.validCount === 0) {
        console.log("StatusPanel: No valid files found");
        dom.showError(
          "No valid audio files found. Please check your files and try again."
        );
        return;
      }

      console.log(
        "StatusPanel: Files validated, getting output configuration..."
      );

      // Get output configuration
      let outputConfig: OutputConfig;
      try {
        outputConfig = getCurrentOutputConfig();
        console.log("StatusPanel: Output configuration retrieved:", outputConfig);
      } catch (error) {
        console.log("StatusPanel: Settings validation failed:", error);
        dom.showError(`Settings validation failed: ${error}`);
        return;
      }

      // Update UI to processing state
      this.isProcessing = true;
      this.updateStatus({
        stage: "analyzing",
        percentage: 0,
        message: "Starting processing...",
      });

      // Disable job controls
      setJobControlsEnabled(false);

      // Update art thumbnail with current file's cover art
      await this.updateArtThumbnail();

      // Start listening for progress events
      await this.startProgressListener();

      // Get file paths for processing
      const filePaths = currentFileList.files
        .filter((file) => file.isValid)
        .map((file) => file.path);

      // Get metadata from the form (basic implementation)
      const metadata = this.getCurrentMetadata();

      // Call backend processing command
      const fallbackEncoderDefaults = (() => {
        const defaults = defaultEncoderSettings();
        const selected = outputConfig.encoderSettings;
        const bitrate = SUPPORTED_ENCODER_BITRATES.has(selected.bitrateKbps)
          ? selected.bitrateKbps
          : defaults.bitrateKbps;
        const channels = selected.channels ?? defaults.channels;
        return {
          ...defaults,
          ...selected,
          bitrateKbps: bitrate,
          channels,
        } satisfies EncoderSettings;
      })();

      const windowWithProvider = window as WindowWithEncoderProvider;
      const providerResult: EncoderSettingsLike =
        typeof windowWithProvider.EncoderSettingsProvider === "function"
          ? windowWithProvider.EncoderSettingsProvider()
          : undefined;

      let boundaryEncoderSettings = toBoundaryEncoderSettings(
        providerResult,
        fallbackEncoderDefaults
      );

      const v2Payload = {
        inputFiles: filePaths,
        outputDir: outputConfig.outputPath,
        settings: boundaryEncoderSettings,
        sampleRate: outputConfig.sampleRate,
        jobType: getJobType(),
      };

      const result = await bridge.invoke<{
        message: string;
        previewFilePath?: string;
        previewActualSeconds?: number;
        jobId?: string;
      }>("process_audiobook_files_v2", {
        payload: v2Payload,
        metadata: Object.keys(metadata).length > 0 ? metadata : null,
        previewSeconds: options?.previewSeconds,
      });

      console.log("Processing completed successfully:", result);
      if (result && result.previewFilePath) {
        const seconds =
          typeof result.previewActualSeconds === "number"
            ? result.previewActualSeconds.toFixed(3)
            : "≈30";
        console.log(
          `Preview file created at: ${result.previewFilePath} (${seconds}s)`
        );
        try {
          await bridge.openExternal(result.previewFilePath);
        } catch (e) {
          console.warn("Failed to open preview file automatically:", e);
        }
      }
      if (options?.previewSeconds) {
        // Optionally handle showing/opening preview file via result once backend returns a path shape
        // Placeholder: UI messaging handled by progress events for now
      }
    } catch (error) {
      const msg = String((error as any)?.message ?? error ?? "");
      if (msg.toLowerCase().includes("cancelled")) {
        return;
      }
      console.error("Processing failed:", error);
      dom.showError(`Processing failed: ${msg}`);
      this.resetToIdle();
    }
  }

  private async startProgressListener(): Promise<void> {
    if (this.cancelUnlisten) {
      this.cancelUnlisten();
    }

    this.cancelUnlisten = await bridge.listen(EVENTS.PROGRESS, (event) => {
      const progress = event.payload as ProcessingProgressEvent;
      this.updateProgress(progress);
    });
  }

  public updateProgress(event: ProcessingProgressEvent): void {
    const jobId = event.job_id ?? "default";
    const now = Date.now();

    // Track per-job progress
    this.jobProgress.set(jobId, {
      jobId,
      stage: event.stage,
      percentage: Math.round(event.percentage * 10) / 10,
      message: event.message,
      lastUpdate: now,
    });

    this.syncJobListUI();

    // Handle terminal states for this job
    if (
      event.stage === STAGES.completed ||
      event.stage === STAGES.failed ||
      event.stage === STAGES.cancelled
    ) {
      // Remove completed/failed/cancelled jobs after a delay
      setTimeout(() => {
        this.jobProgress.delete(jobId);
        if (this.jobProgress.size === 0) {
          this.resetToIdle();

          if (event.stage === STAGES.completed) {
            dom.showSuccess("Audiobook created successfully!");
          } else if (event.stage === STAGES.failed) {
            dom.showError(event.message);
          } else if (event.stage === STAGES.cancelled) {
            dom.showInfo("Processing was cancelled.");
          }
        }

        this.updateAggregateUI();
        this.syncJobListUI();
      }, 2000);
    }

    this.isProcessing = this.jobProgress.size > 0;

    // Calculate aggregate progress
    const aggregate = this.calculateAggregateProgress();

    // Derive status from aggregate (use most advanced active stage)
    const status: ProcessingStatus = {
      stage: this.deriveAggregateStage(),
      percentage: aggregate.overallPercentage,
      message: this.deriveAggregateMessage(aggregate),
      currentFile: event.current_file,
      etaSeconds: event.eta_seconds,
    };

    this.updateStatus(status);

  }

  /** Calculate aggregate progress across all active jobs */
  private calculateAggregateProgress(): AggregateProgress {
    let activeJobs = 0;
    let completedJobs = 0;
    let totalPercentage = 0;

    for (const job of this.jobProgress.values()) {
      if (job.stage === "completed") {
        completedJobs++;
        totalPercentage += 100;
      } else if (job.stage === "failed" || job.stage === "cancelled") {
        // Don't count failed/cancelled in active or completed
      } else {
        activeJobs++;
        totalPercentage += job.percentage;
      }
    }

    const totalJobs = activeJobs + completedJobs;
    // Simple average across active + completed jobs. This keeps the aggregate legible
    // without over-weighting long-running jobs; consider a weighted strategy if we
    // need time/progress proportionality later.
    const overallPercentage = totalJobs > 0 ? totalPercentage / totalJobs : 0;

    return {
      activeJobs,
      completedJobs,
      overallPercentage: Math.round(overallPercentage * 10) / 10,
    };
  }

  /** Render the job list with per-job cancel controls */
  private syncJobListUI(): void {
    const jobs = Array.from(this.jobProgress.values())
      .sort((a, b) => b.lastUpdate - a.lastUpdate)
      .map((job) => ({
        id: job.jobId,
        label: `${job.jobId.slice(0, 8)} • ${job.message}`,
        stage: job.stage,
        percentage: job.percentage,
        onCancel: job.jobId === "default" ? undefined : (id: string) => this.cancelJob(id),
      }));

    dom.renderJobList(jobs);
  }

  /** Derive aggregate stage from active jobs */
  private deriveAggregateStage(): ProcessingStatus["stage"] {
    const stages = Array.from(this.jobProgress.values()).map((j) => j.stage);

    // Priority: failed > cancelled > converting > analyzing > writing > completed > idle
    if (stages.includes("failed")) return "failed";
    if (stages.includes("cancelled")) return "cancelled";
    if (stages.includes("converting")) return "converting";
    if (stages.includes("analyzing")) return "analyzing";
    if (stages.includes("writing")) return "writing";
    if (stages.includes("completed")) return "completed";
    return "idle";
  }

  /** Derive message for aggregate display */
  private deriveAggregateMessage(aggregate: AggregateProgress): string {
    if (aggregate.activeJobs > 1) {
      return `Processing ${aggregate.activeJobs} files (${aggregate.completedJobs} completed)`;
    } else if (aggregate.activeJobs === 1) {
      // Return the single job's message
      const activeJob = Array.from(this.jobProgress.values()).find(
        (j) =>
          j.stage !== "completed" &&
          j.stage !== "failed" &&
          j.stage !== "cancelled"
      );
      return activeJob?.message ?? "Processing...";
    }
    return "Ready to process audiobook";
  }

  /** Update UI with aggregate progress (called after job removal) */
  private updateAggregateUI(): void {
    if (this.jobProgress.size === 0 && !this.isProcessing) {
      return; // No need to update if idle
    }
    this.isProcessing = this.jobProgress.size > 0;
    const aggregate = this.calculateAggregateProgress();
    const status: ProcessingStatus = {
      stage: this.deriveAggregateStage(),
      percentage: aggregate.overallPercentage,
      message: this.deriveAggregateMessage(aggregate),
    };
    this.updateStatus(status);
  }

  private updateStatus(status: ProcessingStatus): void {
    this.currentStatus = status;
    this.updateUI();
  }

  private updateUI(): void {
    // Update progress bar and percentage
    dom.updateProgressBar(this.currentStatus.percentage);
    dom.updatePercentageText(this.currentStatus.percentage);

    // Update status text
    const statusDisplay = this.getStatusDisplayText();
    dom.updateStatusText(statusDisplay);

    // Update step text
    dom.updateStepText(`Current Step: ${this.currentStatus.message}`);

    // Update process button
    dom.updateProcessButton(this.isProcessing);
  }

  private getStatusDisplayText(): string {
    switch (this.currentStatus.stage) {
      case "idle":
        return "Idle";
      case "analyzing":
        return "Analyzing";
      case "converting":
        return "Converting";
      case "writing":
        return "Writing Metadata";
      case "completed":
        return "Completed";
      case "cancelled":
        return "Cancelled";
      case "failed":
        return "Failed";
      default:
        return "Processing";
    }
  }

  private async handleCancelAll(): Promise<void> {
    try {
      await bridge.invoke("cancel_processing");
      // Do not set final cancelled state here; wait for backend events
      this.updateStatus({
        stage: this.currentStatus.stage,
        percentage: this.currentStatus.percentage,
        message: "Cancellation requested…",
      });
    } catch (error) {
      console.error("Failed to cancel processing:", error);
      dom.showError("Failed to cancel processing. Please try again.");
    }
  }

  private async cancelJob(jobId: string): Promise<void> {
    try {
      await bridge.invoke("cancel_processing", { job_id: jobId });
    } catch (error) {
      console.error(`Failed to cancel job ${jobId}:`, error);
      dom.showError(`Failed to cancel job ${jobId}`);
    }
  }

  private resetToIdle(): void {
    this.isProcessing = false;

    if (this.cancelUnlisten) {
      this.cancelUnlisten();
      this.cancelUnlisten = undefined;
    }

    // Clear all job progress tracking
    this.jobProgress.clear();
    dom.renderJobList([]);

    this.updateStatus({
      stage: "idle",
      percentage: 0,
      message: "Ready to process audiobook",
    });

    // Re-enable controls
    setJobControlsEnabled(true);

    // Reset art thumbnail to placeholder
    dom.resetArtThumbnail();
  }

  private convertBytesToDataUrl(bytes: number[]): string {
    // Convert number array to Uint8Array
    const uint8Array = new Uint8Array(bytes);

    // Detect image format from magic bytes
    let mimeType = "image/jpeg"; // default fallback
    if (uint8Array.length >= 4) {
      // PNG: 89 50 4E 47
      if (
        uint8Array[0] === 0x89 &&
        uint8Array[1] === 0x50 &&
        uint8Array[2] === 0x4e &&
        uint8Array[3] === 0x47
      ) {
        mimeType = "image/png";
      }
      // JPEG: FF D8 FF
      else if (
        uint8Array[0] === 0xff &&
        uint8Array[1] === 0xd8 &&
        uint8Array[2] === 0xff
      ) {
        mimeType = "image/jpeg";
      }
      // WebP: 52 49 46 46 ... 57 45 42 50
      else if (
        uint8Array[0] === 0x52 &&
        uint8Array[1] === 0x49 &&
        uint8Array[2] === 0x46 &&
        uint8Array[3] === 0x46
      ) {
        mimeType = "image/webp";
      }
    }

    // Convert to base64
    let binary = "";
    uint8Array.forEach((byte) => {
      binary += String.fromCharCode(byte);
    });
    const base64 = btoa(binary);

    return `data:${mimeType};base64,${base64}`;
  }

  private async updateArtThumbnail(): Promise<void> {
    if (!currentFileList || !currentFileList.files.length) {
      dom.resetArtThumbnail();
      return;
    }

    // Get the first valid file for cover art
    const firstValidFile = currentFileList.files.find((f) => f.isValid);
    if (!firstValidFile) {
      dom.resetArtThumbnail();
      return;
    }

    try {
      // Load metadata with cover art
      const metadata = await bridge.invoke<AudiobookMetadata>(
        "read_audio_metadata",
        {
          filePath: firstValidFile.path,
        }
      );

      // Check for cover art data (backend returns as cover_art field with number array)
      if (metadata.cover_art && metadata.cover_art.length > 0) {
        const dataUrl = this.convertBytesToDataUrl(metadata.cover_art);
        dom.displayCoverArt(dataUrl);
      } else {
        dom.resetArtThumbnail();
      }
    } catch (error) {
      console.warn("Failed to load cover art for thumbnail:", error);
      dom.resetArtThumbnail();
    }
  }

  private getCurrentMetadata(): Partial<AudiobookMetadata> {
    // Basic metadata extraction from DOM elements
    const getElementValue = (id: string): string => {
      const element = document.getElementById(id) as HTMLInputElement;
      return element?.value?.trim() || "";
    };

    const metadata: Partial<AudiobookMetadata> = {};

    const title = getElementValue("meta-title");
    const author = getElementValue("meta-author");
    const narrator = getElementValue("meta-narrator");
    const year = getElementValue("meta-year");
    const genre = getElementValue("meta-genre");
    const series = getElementValue("meta-series");
    const seriesPart = getElementValue("meta-series-part");
    const description = getElementValue("meta-description");

    if (title) {
      metadata.title = title;
      metadata.album = title; // Album derived from title
    }
    if (author) metadata.artist = author; // Map author -> artist for backend
    if (narrator) metadata.composer = narrator; // Map narrator -> composer for backend
    if (year) {
      const yearNum = parseInt(year);
      if (!isNaN(yearNum)) metadata.date = yearNum; // Map year -> date for backend
    }
    if (genre) metadata.genre = genre;
    if (series) {
      // TODO: Persist MVNM (series name) when backend supports it
      // For now, append to album if series is provided
      if (metadata.album) {
        metadata.album = `${metadata.album} (${series}${seriesPart ? " " + seriesPart : ""
          })`;
      }
    }
    if (description) metadata.description = description;

    // Include cover art if user has selected any
    const coverBytes = getCurrentCoverArt();
    if (coverBytes && coverBytes.length > 0) {
      metadata.cover_art = coverBytes;
    }

    return metadata;
  }

  // Public method to check if processing is active
  public get isCurrentlyProcessing(): boolean {
    return this.isProcessing;
  }

  // Public method to get current status
  public getCurrentStatus(): ProcessingStatus {
    return { ...this.currentStatus };
  }
}

// Export a singleton instance
let statusPanelInstance: StatusPanel | null = null;

export function initStatusPanel(): StatusPanel {
  if (!statusPanelInstance) {
    statusPanelInstance = new StatusPanel();
  }
  return statusPanelInstance;
}

export function getStatusPanel(): StatusPanel | null {
  return statusPanelInstance;
}
