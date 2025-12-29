/**
 * StatusPanel business logic and state management
 *
 * This module contains the core StatusPanel class with event handling,
 * processing coordination, and state management.
 */

import { bridge } from "../../lib/bridge";
import { STAGES } from "../../types/events";
import type { ProcessingProgressEvent } from "../../types/events";
import { currentFileList, setFileOrderLocked } from "../fileList";
import { AudiobookMetadata } from "../../types/metadata";
import * as dom from "./dom";
import { setJobControlsEnabled } from "../jobControls";
import { bindStatusPanelDomEvents, listenForProgressEvents } from "./events";
import {
  convertBytesToDataUrl,
  extractFilenameFromProgress,
  formatAggregateMessage,
} from "./formatting";
import { startProcessing as startProcessingAction } from "./processing";
import { renderConcurrencyStatus, renderJobList, renderStatus } from "./render";
import {
  calculateAggregateProgress as calculateAggregateProgressState,
  createInitialStatus,
  deriveAggregateStage as deriveAggregateStageState,
  type AggregateProgress,
  type JobProgress,
  type ProcessingStatus,
} from "./state";

export class StatusPanel {
  private cancelUnlisten?: () => void;
  private isProcessing: boolean = false;
  private currentStatus: ProcessingStatus;
  private previewDuration: number = 30;
  /** Per-job progress tracking for parallel batch processing */
  private jobProgress: Map<string, JobProgress> = new Map();
  private lastProgressRender = 0;
  private currentJobType: "merge" | "batch" | null = null;
  private lastCoverArtPath: string | null = null;

  constructor() {
    this.currentStatus = createInitialStatus();

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
    this.updateConcurrencyIndicator();

    // Initialize art thumbnail to placeholder
    dom.resetArtThumbnail();
  }

  private setupEventHandlers(): void {
    bindStatusPanelDomEvents({
      onProcess: () => this.startProcessing(),
      onCancelAll: () => this.handleCancelAll(),
      onPreview: (duration) =>
        this.startProcessing({ previewSeconds: duration }),
      getPreviewDuration: () => this.previewDuration,
      setPreviewDuration: (duration) => {
        this.previewDuration = duration;
      },
      onUpdateConcurrencyIndicator: () => this.updateConcurrencyIndicator(),
    });
  }

  // MaxConcurrent control moved to src/ui/jobControls.ts

  public async startProcessing(options?: {
    previewSeconds?: number;
  }): Promise<void> {
    return startProcessingAction(
      {
        updateStatus: (status) => this.updateStatus(status),
        setProcessingState: (isProcessing) => {
          this.isProcessing = isProcessing;
        },
        updateArtThumbnail: () => this.updateArtThumbnail(),
        startProgressListener: () => this.startProgressListener(),
        setCurrentJobType: (jobType) => {
          this.currentJobType = jobType;
        },
        resetToIdle: () => this.resetToIdle(),
      },
      options
    );
  }

  private async startProgressListener(): Promise<void> {
    if (this.cancelUnlisten) {
      this.cancelUnlisten();
    }

    this.cancelUnlisten = await listenForProgressEvents((progress) => {
      this.updateProgress(progress);
    });
  }

  public updateProgress(event: ProcessingProgressEvent): void {
    const jobId = event.job_id ?? "default";
    const now = Date.now();

    // Throttle non-terminal updates to avoid UI flooding with many jobs
    const isTerminal =
      event.stage === STAGES.completed ||
      event.stage === STAGES.failed ||
      event.stage === STAGES.cancelled;
    if (!isTerminal && now - this.lastProgressRender < 500) {
      return;
    }
    this.lastProgressRender = now;

    // Track per-job progress
    this.jobProgress.set(jobId, {
      jobId,
      stage: event.stage,
      percentage: Math.round(event.percentage * 10) / 10,
      message: event.message,
      lastUpdate: now,
    });

    renderJobList(this.jobProgress, (id) => this.cancelJob(id));

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
        renderJobList(this.jobProgress, (id) => this.cancelJob(id));
      }, 2000);
    }

    this.isProcessing = this.jobProgress.size > 0;

    // Calculate aggregate progress
    const aggregate = this.calculateAggregateProgress();
    this.updateConcurrencyIndicator(aggregate);

    // Derive status from aggregate (use most advanced active stage)
    const status: ProcessingStatus = {
      stage: this.deriveAggregateStage(),
      percentage: aggregate.overallPercentage,
      message: formatAggregateMessage(this.jobProgress, aggregate),
      currentFile: event.current_file,
      etaSeconds: event.eta_seconds,
    };

    this.updateStatus(status);

    if (this.currentJobType === "batch") {
      const indexedPath =
        typeof event.input_index === "number"
          ? this.findFilePathByIndex(event.input_index)
          : null;
      if (indexedPath) {
        void this.updateArtThumbnailForFile(indexedPath);
      } else if (event.current_file) {
        const filename = extractFilenameFromProgress(event.current_file);
        if (filename) {
          const filePath = this.findFilePathByName(filename);
          if (filePath) {
            void this.updateArtThumbnailForFile(filePath);
          }
        }
      }
    }
  }

  /** Calculate aggregate progress across all active jobs */
  private calculateAggregateProgress(): AggregateProgress {
    return calculateAggregateProgressState(this.jobProgress);
  }

  /** Derive aggregate stage from active jobs */
  private deriveAggregateStage(): ProcessingStatus["stage"] {
    return deriveAggregateStageState(this.jobProgress);
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
      message: formatAggregateMessage(this.jobProgress, aggregate),
    };
    this.updateStatus(status);
    this.updateConcurrencyIndicator(aggregate);
  }

  private updateConcurrencyIndicator(aggregate?: AggregateProgress): void {
    renderConcurrencyStatus(aggregate);
  }

  private updateStatus(status: ProcessingStatus): void {
    this.currentStatus = status;
    this.updateUI();
  }

  private updateUI(): void {
    renderStatus(this.currentStatus, this.isProcessing);
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
    this.currentJobType = null;
    this.lastCoverArtPath = null;

    if (this.cancelUnlisten) {
      this.cancelUnlisten();
      this.cancelUnlisten = undefined;
    }

    // Clear all job progress tracking
    this.jobProgress.clear();
    renderJobList(this.jobProgress, (id) => this.cancelJob(id));

    this.updateStatus(createInitialStatus());
    this.updateConcurrencyIndicator();

    // Re-enable controls
    setJobControlsEnabled(true);
    setFileOrderLocked(false);

    // Reset art thumbnail to placeholder
    dom.resetArtThumbnail();
  }

  private async updateArtThumbnail(): Promise<void> {
    if (!currentFileList || !currentFileList.files.length) {
      dom.resetArtThumbnail();
      return;
    }

    // Get the first valid file for cover art
    const firstValidFile = currentFileList.files.find((file) => file.isValid);
    if (!firstValidFile) {
      dom.resetArtThumbnail();
      return;
    }

    await this.updateArtThumbnailForFile(firstValidFile.path);
  }

  private async updateArtThumbnailForFile(filePath: string): Promise<void> {
    if (this.lastCoverArtPath === filePath) {
      return;
    }
    this.lastCoverArtPath = filePath;

    try {
      const metadata = await bridge.invoke<AudiobookMetadata>(
        "read_audio_metadata",
        {
          filePath,
        }
      );

      if (metadata.cover_art && metadata.cover_art.length > 0) {
        const dataUrl = convertBytesToDataUrl(metadata.cover_art);
        dom.displayCoverArt(dataUrl);
      } else {
        dom.resetArtThumbnail();
      }
    } catch (error) {
      console.warn("Failed to load cover art for thumbnail:", error);
      dom.resetArtThumbnail();
    }
  }

  private findFilePathByName(filename: string): string | null {
    if (!currentFileList) return null;
    const match = currentFileList.files.find((file) => {
      const base = file.path.split(/[\\/]/).pop() || "";
      return base === filename;
    });
    return match?.path ?? null;
  }

  private findFilePathByIndex(index: number): string | null {
    if (!currentFileList) return null;
    if (!Number.isInteger(index)) return null;
    if (index < 0 || index >= currentFileList.files.length) return null;
    return currentFileList.files[index]?.path ?? null;
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
