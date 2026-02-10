import { bridge } from "../../lib/bridge";
import type { EncoderSettings, OutputConfig } from "../../types/audio";
import {
  defaultEncoderSettings,
  VALID_ENCODER_BITRATES,
} from "../../types/audio";
import { toBoundaryEncoderSettings } from "../../types/encoder";
import type { EncoderSettingsLike } from "../../types/encoder";
import type { AudiobookMetadata } from "../../types/metadata";
import {
  currentFileList,
  selectedFileIndex,
  setFileOrderLocked,
} from "../fileList";
import { getSelectedFileIndices } from "../fileList/state";
import { getCurrentOutputConfig } from "../outputPanel";
import { getJobType, setJobControlsEnabled } from "../jobControls";
import { readMetadataForm } from "../metadataForm";
import {
  getAllMetadata,
  getMetadataForFile,
  setMetadataForFile,
} from "../metadataState";
import { stageMetadataToSelection } from "../fileList/actions";
import * as dom from "./dom";
import type { ProcessingStatus } from "./state";

interface WindowWithEncoderProvider extends Window {
  EncoderSettingsProvider?: () => EncoderSettingsLike;
}

interface StartProcessingContext {
  updateStatus: (status: ProcessingStatus) => void;
  setProcessingState: (isProcessing: boolean) => void;
  updateArtThumbnail: () => Promise<void>;
  startProgressListener: () => Promise<void>;
  setCurrentJobType: (jobType: "merge" | "batch") => void;
  resetToIdle: () => void;
}

// Derived from centralized VALID_ENCODER_BITRATES (audio.ts)
// Typed as Set<number> to allow membership check with any numeric bitrate
const SUPPORTED_ENCODER_BITRATES: Set<number> = new Set(VALID_ENCODER_BITRATES);

export async function startProcessing(
  context: StartProcessingContext,
  options?: {
    previewSeconds?: number;
  }
): Promise<void> {
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
    context.setProcessingState(true);
    context.updateStatus({
      stage: "analyzing",
      percentage: 0,
      message: "Starting processing...",
    });

    // Disable job controls
    setJobControlsEnabled(false);
    setFileOrderLocked(true);

    // Update art thumbnail with current file's cover art
    await context.updateArtThumbnail();

    // Start listening for progress events
    await context.startProgressListener();

    // Get file paths for processing
    const filePaths = currentFileList.files
      .filter((file) => file.isValid)
      .map((file) => file.path);

    const selectionCount = getSelectedFileIndices().size;
    if (selectionCount > 1) {
      await stageMetadataToSelection({ showStatus: false });
    }
    let currentMetadata: Partial<AudiobookMetadata> = {};
    if (selectionCount <= 1) {
      currentMetadata = readMetadataForm({ mode: "single" });
      const activeFile =
        selectedFileIndex >= 0
          ? currentFileList.files[selectedFileIndex]
          : currentFileList.files.find((file) => file.isValid);
      if (activeFile?.isValid) {
        setMetadataForFile(activeFile.path, currentMetadata);
      }
    }

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

    const boundaryEncoderSettings = toBoundaryEncoderSettings(
      providerResult,
      fallbackEncoderDefaults
    );

    const jobType = getJobType();
    context.setCurrentJobType(jobType);

    const v2Payload = {
      inputFiles: filePaths,
      outputDir: outputConfig.outputPath,
      settings: boundaryEncoderSettings,
      sampleRate: outputConfig.sampleRate,
      jobType,
      outputNaming: outputConfig.outputNaming,
    };

    if (v2Payload.jobType === "batch") {
      const missingMetadata = filePaths.filter(
        (filePath) => !getMetadataForFile(filePath)
      );
      if (missingMetadata.length > 0) {
        await Promise.all(
          missingMetadata.map(async (filePath) => {
            try {
              const metadata = await bridge.invoke(
                "read_audio_metadata",
                { filePath }
              );
              setMetadataForFile(filePath, metadata);
            } catch (error) {
              console.warn(
                "Failed to load metadata for batch file:",
                filePath,
                error
              );
            }
          })
        );
      }
    }

    let metadataPayload: Record<string, Partial<AudiobookMetadata>> | null =
      null;
    if (v2Payload.jobType === "merge") {
      if (
        v2Payload.inputFiles.length > 0 &&
        Object.keys(currentMetadata).length > 0
      ) {
        metadataPayload = {
          [v2Payload.inputFiles[0]]: currentMetadata,
        };
      }
    } else {
      const storedMetadata = getAllMetadata();
      const filteredMetadata = Object.fromEntries(
        Object.entries(storedMetadata).filter(
          ([, value]) => Object.keys(value).length > 0
        )
      );
      metadataPayload =
        Object.keys(filteredMetadata).length > 0 ? filteredMetadata : null;
    }

    const result = await bridge.invoke("process_audiobook_files_v2", {
      payload: v2Payload,
      metadata: metadataPayload,
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
      } catch (error) {
        console.warn("Failed to open preview file automatically:", error);
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
    context.resetToIdle();
  }
}
