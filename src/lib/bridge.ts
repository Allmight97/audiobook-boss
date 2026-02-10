/// <reference types="vite/client" />
import { listen as tauriListen, UnlistenFn } from "@tauri-apps/api/event";
import { open as tauriOpen, OpenDialogOptions } from "@tauri-apps/plugin-dialog";
import { openPath as tauriOpenExternal } from "@tauri-apps/plugin-opener";

import {
  commands as generatedCommands,
  events as generatedEvents,
  type AudiobookMetadata as GeneratedAudiobookMetadata,
  type AudioFile as GeneratedAudioFile,
  type FileListInfo as GeneratedFileListInfo,
  type MetadataSource as GeneratedMetadataSource,
  type OnlineMetadataResult as GeneratedOnlineMetadataResult,
  type ProcessCommandResult as GeneratedProcessCommandResult,
  type ProcessV2Payload as GeneratedProcessV2Payload,
  type ProgressEvent as GeneratedProgressEvent,
  type QueueEvent as GeneratedQueueEvent,
} from "./generated/tauri";
import type { ApplicationEvents, EventName, ProcessingProgressEvent, ProcessingQueueEvent } from "../types/events";
import type {
  AudioFile as LegacyAudioFile,
  EncoderSettings,
  FileListInfo as LegacyFileListInfo,
  ProcessCommandResult as LegacyProcessCommandResult,
  ProcessV2Payload as LegacyProcessV2Payload,
} from "../types/audio";
import type {
  AudiobookMetadata as LegacyAudiobookMetadata,
  MetadataSource as LegacyMetadataSource,
  OnlineMetadataResult as LegacyOnlineMetadataResult,
} from "../types/metadata";

// Check if we are running in a Tauri environment
const isTauri = !!(window as any).__TAURI_INTERNALS__;
console.log(`[Bridge] Initialized. isTauri=${isTauri}, DEV=${import.meta.env.DEV}`);

const toOptional = <T>(value: T | null | undefined): T | undefined =>
  value == null ? undefined : value;

function normalizeMetadata(metadata: GeneratedAudiobookMetadata): LegacyAudiobookMetadata {
  return {
    title: toOptional(metadata.title),
    artist: toOptional(metadata.artist),
    album: toOptional(metadata.album),
    composer: toOptional(metadata.composer),
    genre: toOptional(metadata.genre),
    date: toOptional(metadata.date),
    track: toOptional(metadata.track),
    disk: toOptional(metadata.disk),
    comment: toOptional(metadata.comment),
    description: toOptional(metadata.description),
    series: toOptional(metadata.series),
    series_part: toOptional(metadata.series_part),
    subseries: toOptional(metadata.subseries),
    subseries_part: toOptional(metadata.subseries_part),
    album_sort: toOptional(metadata.album_sort),
    cover_art: toOptional(metadata.cover_art),
  };
}

function denormalizeMetadata(
  metadata: Partial<LegacyAudiobookMetadata>
): GeneratedAudiobookMetadata {
  return {
    title: metadata.title ?? null,
    artist: metadata.artist ?? null,
    album: metadata.album ?? null,
    composer: metadata.composer ?? null,
    genre: metadata.genre ?? null,
    date: metadata.date ?? null,
    track: metadata.track ?? null,
    disk: metadata.disk ?? null,
    comment: metadata.comment ?? null,
    description: metadata.description ?? null,
    series: metadata.series ?? null,
    series_part: metadata.series_part ?? null,
    subseries: metadata.subseries ?? null,
    subseries_part: metadata.subseries_part ?? null,
    album_sort: metadata.album_sort ?? null,
    cover_art: metadata.cover_art ?? null,
  };
}

function normalizeAudioFile(file: GeneratedAudioFile): LegacyAudioFile {
  return {
    path: file.path,
    size: toOptional(file.size),
    duration: toOptional(file.duration),
    format: toOptional(file.format),
    bitrate: toOptional(file.bitrate),
    sampleRate: toOptional(file.sampleRate),
    channels: toOptional(file.channels),
    isValid: file.isValid,
    error: toOptional(file.error),
  };
}

function normalizeFileList(info: GeneratedFileListInfo): LegacyFileListInfo {
  return {
    files: info.files.map(normalizeAudioFile),
    totalDuration: info.totalDuration,
    totalSize: info.totalSize,
    validCount: info.validCount,
    invalidCount: info.invalidCount,
  };
}

function normalizeLookupResult(
  result: GeneratedOnlineMetadataResult
): LegacyOnlineMetadataResult {
  return {
    source: result.source as LegacyMetadataSource,
    sourceId: result.sourceId,
    title: result.title,
    authors: result.authors,
    narrators: result.narrators,
    series: toOptional(result.series),
    seriesPart: toOptional(result.seriesPart),
    subseries: toOptional(result.subseries),
    subseriesPart: toOptional(result.subseriesPart),
    description: toOptional(result.description),
    publishedYear: toOptional(result.publishedYear),
    durationSeconds: toOptional(result.durationSeconds),
    coverUrl: toOptional(result.coverUrl),
    audibleOnly: toOptional(result.audibleOnly),
  };
}

function denormalizeProcessPayload(
  payload: LegacyProcessV2Payload
): GeneratedProcessV2Payload {
  return {
    inputFiles: payload.inputFiles,
    outputDir: payload.outputDir,
    settings: payload.settings,
    sampleRate: payload.sampleRate ?? null,
    jobType: payload.jobType ?? null,
    outputNaming: payload.outputNaming ?? null,
  };
}

function normalizeProcessResult(
  result: GeneratedProcessCommandResult
): LegacyProcessCommandResult & { previewActualSeconds?: number; jobId: string } {
  return {
    message: result.message,
    previewFilePath: toOptional(result.previewFilePath),
    previewActualSeconds: toOptional(result.previewActualSeconds),
    jobId: result.jobId,
  };
}

function normalizeProgressEvent(payload: GeneratedProgressEvent): ProcessingProgressEvent {
  return {
    stage: payload.stage as ProcessingProgressEvent["stage"],
    percentage: payload.percentage,
    message: payload.message,
    current_file: toOptional(payload.current_file),
    eta_seconds: toOptional(payload.eta_seconds),
    job_id: toOptional(payload.job_id),
    input_index: toOptional(payload.input_index),
  };
}

function normalizeQueueEvent(payload: GeneratedQueueEvent): ProcessingQueueEvent {
  return {
    items: payload.items.map((item) => ({
      input_index: item.input_index,
      file_path: item.file_path,
    })),
    max_concurrent: payload.max_concurrent,
  };
}

type LegacyMetadataPayload = Record<string, Partial<LegacyAudiobookMetadata>>;

const commandInvokers = {
  ping: (_args?: undefined) => generatedCommands.ping(),
  echo: (args: { input: string }) => generatedCommands.echo(args.input),
  validate_files: (args: { filePaths: string[] }) =>
    generatedCommands.validateFiles(args.filePaths),
  read_audio_metadata: (args: { filePath: string }) =>
    generatedCommands.readAudioMetadata(args.filePath).then(normalizeMetadata),
  write_cover_art: (args: { filePath: string; coverData: number[] }) =>
    generatedCommands.writeCoverArt(args.filePath, args.coverData),
  load_cover_art_file: (args: { filePath: string }) =>
    generatedCommands.loadCoverArtFile(args.filePath),
  load_cover_art_from_url: (args: { url: string }) =>
    generatedCommands.loadCoverArtFromUrl(args.url),
  save_metadata_to_file: (args: {
    filePath: string;
    metadata: Partial<LegacyAudiobookMetadata>;
  }) => generatedCommands.saveMetadataToFile(args.filePath, denormalizeMetadata(args.metadata)),
  search_online_metadata: (args: {
    query: string;
    sources?: LegacyMetadataSource[] | null;
    limit?: number | null;
  }) =>
    generatedCommands
      .searchOnlineMetadata(
        args.query,
        (args.sources ?? null) as GeneratedMetadataSource[] | null,
        args.limit ?? null
      )
      .then((results) => results.map(normalizeLookupResult)),
  analyze_audio_files: (args: { filePaths: string[] }) =>
    generatedCommands.analyzeAudioFiles(args.filePaths).then(normalizeFileList),
  validate_encoder_settings_cmd: (args: { settings: EncoderSettings }) =>
    generatedCommands.validateEncoderSettingsCmd(args.settings),
  list_available_encoders: (_args?: undefined) =>
    generatedCommands.listAvailableEncoders(),
  get_max_concurrent_jobs: (_args?: undefined) =>
    generatedCommands.getMaxConcurrentJobs(),
  set_max_concurrent_jobs: (args: { max_concurrent?: number | null }) =>
    generatedCommands.setMaxConcurrentJobs(args.max_concurrent ?? null),
  process_audiobook_files_v2: (args: {
    payload: LegacyProcessV2Payload;
    metadata?: LegacyMetadataPayload | null;
    previewSeconds?: number | null;
  }) => {
    const metadataPayload = args.metadata
      ? Object.fromEntries(
          Object.entries(args.metadata).map(([path, value]) => [
            path,
            denormalizeMetadata(value),
          ])
        )
      : null;

    return generatedCommands
      .processAudiobookFilesV2(
        denormalizeProcessPayload(args.payload),
        metadataPayload,
        args.previewSeconds ?? null
      )
      .then(normalizeProcessResult);
  },
  cancel_processing: (args?: { job_id?: string | null }) =>
    generatedCommands.cancelProcessing(args?.job_id ?? null),
} as const;

type BridgeCommand = keyof typeof commandInvokers;
type BridgeCommandArgs<K extends BridgeCommand> = Parameters<
  (typeof commandInvokers)[K]
>[0];
type BridgeCommandResult<K extends BridgeCommand> = Awaited<
  ReturnType<(typeof commandInvokers)[K]>
>;

// Helper to lazily load mocks only in DEV mode
async function getMocks() {
  console.log("[Bridge] Loading mocks...");
  if (import.meta.env.DEV) {
    return await import("./mocks");
  }
  throw new Error("Mocks are not available in production builds");
}

export const bridge = {
  /**
   * Typed wrapper for Tauri commands while preserving legacy command names at call sites.
   */
  invoke: async <K extends BridgeCommand>(
    cmd: K,
    args?: BridgeCommandArgs<K>
  ): Promise<BridgeCommandResult<K>> => {
    if (isTauri) {
      const command = commandInvokers[cmd];
      return (await command(args as never)) as unknown as BridgeCommandResult<K>;
    }

    if (import.meta.env.DEV) {
      const mocks = await getMocks();
      return (await mocks.mockInvoke(cmd, args as never)) as unknown as BridgeCommandResult<K>;
    }

    console.warn(`[Bridge] Tauri not detected and not in DEV mode. Command '${cmd}' ignored.`);
    return Promise.reject("Tauri API not available") as unknown as BridgeCommandResult<K>;
  },

  /**
   * Typed wrapper for Tauri listen() with generated app events + built-in Tauri events.
   */
  listen: async <E extends EventName>(
    event: E,
    handler: (event: { payload: ApplicationEvents[E] }) => void
  ): Promise<UnlistenFn> => {
    if (isTauri) {
      if (event === "processing-progress") {
        return generatedEvents.processingProgress.listen((evt) => {
          handler({ payload: normalizeProgressEvent(evt.payload) as ApplicationEvents[E] });
        });
      }

      if (event === "processing-queue") {
        return generatedEvents.processingQueue.listen((evt) => {
          handler({ payload: normalizeQueueEvent(evt.payload) as ApplicationEvents[E] });
        });
      }

      return tauriListen(event, handler as never);
    }

    if (import.meta.env.DEV) {
      const mocks = await getMocks();
      return mocks.mockListen(event, handler as never);
    }

    console.warn(`[Bridge] Tauri not detected. Listener for '${event}' ignored.`);
    return () => {};
  },

  /**
   * Wrapper for Tauri's dialog open function
   */
  open: async (options?: OpenDialogOptions): Promise<null | string | string[]> => {
    if (isTauri) {
      return tauriOpen(options);
    }

    if (import.meta.env.DEV) {
      const mocks = await getMocks();
      return mocks.mockOpen(options);
    }

    console.warn("[Bridge] Tauri not detected. Dialog open ignored.");
    return null;
  },

  /**
   * Wrapper for Tauri's open (external) function
   */
  openExternal: async (path: string): Promise<void> => {
    if (isTauri) {
      return tauriOpenExternal(path);
    }

    if (import.meta.env.DEV) {
      const mocks = await getMocks();
      return mocks.mockOpenExternal(path);
    }

    console.warn(`[Bridge] Tauri not detected. External open for '${path}' ignored.`);
  },
};

export const BRIDGE_COMMAND_NAMES = Object.freeze(
  Object.keys(commandInvokers)
) as readonly BridgeCommand[];

export const BRIDGE_APP_EVENT_NAMES = Object.freeze([
  "processing-progress",
  "processing-queue",
] as const);

export type { BridgeCommand };
