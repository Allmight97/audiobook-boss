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
import { getCurrentAudioSettings } from "../outputPanel";
import type { EncoderSettings } from "../../types/audio";
import {
  defaultEncoderSettings,
  VALID_ENCODER_BITRATES,
} from "../../types/audio";
import { toBoundaryEncoderSettings } from "../../types/encoder";
import type { EncoderSettingsLike } from "../../types/encoder";
import { AudiobookMetadata } from "../../types/metadata";
import * as dom from "./dom";
import { getCurrentCoverArt } from "../coverArt";

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

type WindowWithEncoderProvider = Window & {
  EncoderSettingsProvider?: () => EncoderSettingsLike;
};

// Derived from centralized VALID_ENCODER_BITRATES (audio.ts)
// Typed as Set<number> to allow membership check with any numeric bitrate
const SUPPORTED_ENCODER_BITRATES: Set<number> = new Set(VALID_ENCODER_BITRATES);

export class StatusPanel {
  private cancelUnlisten?: () => void;
  private isProcessing: boolean = false;
  private currentStatus: ProcessingStatus;
  private previewDuration: number = 30;

  constructor() {
    this.currentStatus = {
      stage: "idle",
      percentage: 0,
      message: "Ready to process audiobook",
    };

    this.initializeElements();
    this.setupEventHandlers();

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
      processButton.addEventListener(
        "click",
        this.handleProcessButtonClick.bind(this)
      );
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

  private async handleProcessButtonClick(): Promise<void> {
    if (this.isProcessing) {
      // Cancel processing
      await this.handleCancel();
    } else {
      // Start processing
      await this.startProcessing();
    }
  }

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

      console.log("StatusPanel: Files validated, getting audio settings...");

      // Get audio settings
      let settings;
      try {
        settings = getCurrentAudioSettings();
        console.log("StatusPanel: Audio settings retrieved:", settings);
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

      // Call backend processing command (v2 payload)
      const fallbackEncoderDefaults = (() => {
        const defaults = defaultEncoderSettings();
        const bitrate = SUPPORTED_ENCODER_BITRATES.has(settings.bitrate)
          ? (settings.bitrate as EncoderSettings["bitrateKbps"])
          : defaults.bitrateKbps;
        const channels: EncoderSettings["channels"] =
          settings.channels === "Stereo"
            ? "stereo"
            : settings.channels === "Mono"
            ? "mono"
            : "auto";
        return {
          ...defaults,
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
        outputDir:
          (document.getElementById("output-dir-text") as HTMLInputElement)
            ?.value || "",
        settings: boundaryEncoderSettings,
        sampleRate: settings.sampleRate,
      };

      const result = await bridge.invoke<{
        message: string;
        previewFilePath?: string;
        previewActualSeconds?: number;
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
    const status: ProcessingStatus = {
      stage: event.stage,
      percentage: Math.round(event.percentage * 10) / 10, // Round to 1 decimal place
      message: event.message,
      currentFile: event.current_file,
      etaSeconds: event.eta_seconds,
    };

    this.updateStatus(status);

    // Handle completion or failure
    if (status.stage === STAGES.completed) {
      setTimeout(() => {
        this.resetToIdle();
        dom.showSuccess("Audiobook created successfully!");
      }, 2000); // Show success for 2 seconds
    } else if (status.stage === STAGES.failed) {
      this.resetToIdle();
      dom.showError(status.message);
    } else if (status.stage === STAGES.cancelled) {
      this.resetToIdle();
      dom.showInfo("Processing was cancelled.");
    }
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

  private async handleCancel(): Promise<void> {
    try {
      await bridge.invoke("cancel_processing");
      // Do not set final cancelled state here; wait for backend event
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

  private resetToIdle(): void {
    this.isProcessing = false;

    if (this.cancelUnlisten) {
      this.cancelUnlisten();
      this.cancelUnlisten = undefined;
    }

    this.updateStatus({
      stage: "idle",
      percentage: 0,
      message: "Ready to process audiobook",
    });

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
        metadata.album = `${metadata.album} (${series}${
          seriesPart ? " " + seriesPart : ""
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
