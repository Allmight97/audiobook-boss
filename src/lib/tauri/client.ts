import { listen as tauriListen, type UnlistenFn } from '@tauri-apps/api/event';
import {
	open as tauriOpen,
	type OpenDialogOptions,
	type OpenDialogReturn,
} from '@tauri-apps/plugin-dialog';
import { openPath as tauriOpenPath, openUrl as tauriOpenUrl } from '@tauri-apps/plugin-opener';

import { events as generatedEvents } from '../generated/tauri';
import {
	EVENTS,
	type ApplicationEvents,
	type EventName,
	type OpenedAudioFilesEvent,
	type ProcessingProgressEvent,
	type ProcessingQueueEvent,
} from '../../types/events';
import type {
	ExternalToolchainPreference,
	EncoderSettings,
	ProcessPayload,
	FileListInfo,
	ProcessCommandResult,
	ProcessingPreflightPlan,
	OutputKind,
	RuntimeSettingsCapabilities,
} from '../../types/audio';
import type { AppSettings, AppSettingsPatch } from '../../types/appSettings';
import type { AudiobookMetadata, MetadataSaveRequest, MetadataSource } from '../../types/metadata';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import { commandSpecs, type CommandResult, type TauriCommand } from './commands';
import { normalizeProgressEvent, normalizeQueueEvent } from './normalizers';

type MetadataIntentByPath = Record<string, MetadataIntentPatch>;
type AppEventName = (typeof TAURI_APP_EVENT_NAMES)[number];
type RuntimeEventName = Exclude<EventName, AppEventName>;
type ProgressEventHandler = (event: { payload: ProcessingProgressEvent }) => void;
type QueueEventHandler = (event: { payload: ProcessingQueueEvent }) => void;
type OpenedAudioFilesHandler = (event: { payload: OpenedAudioFilesEvent }) => void;
type DialogOptions = Omit<OpenDialogOptions, 'multiple' | 'directory'>;

async function listenProcessingProgress(handler: ProgressEventHandler): Promise<UnlistenFn> {
	return generatedEvents.processingProgress.listen((event) => {
		handler({ payload: normalizeProgressEvent(event.payload) });
	});
}

async function listenProcessingQueue(handler: QueueEventHandler): Promise<UnlistenFn> {
	return generatedEvents.processingQueue.listen((event) => {
		handler({ payload: normalizeQueueEvent(event.payload) });
	});
}

async function listenOpenedAudioFiles(handler: OpenedAudioFilesHandler): Promise<UnlistenFn> {
	return generatedEvents.openedAudioFiles.listen((event) => {
		handler({ payload: event.payload });
	});
}

function listen(event: typeof EVENTS.PROGRESS, handler: ProgressEventHandler): Promise<UnlistenFn>;
function listen(event: typeof EVENTS.QUEUE, handler: QueueEventHandler): Promise<UnlistenFn>;
function listen(
	event: typeof EVENTS.OPENED_AUDIO_FILES,
	handler: OpenedAudioFilesHandler,
): Promise<UnlistenFn>;
function listen<E extends RuntimeEventName>(
	event: E,
	handler: (event: { payload: ApplicationEvents[E] }) => void,
): Promise<UnlistenFn>;
function listen(
	event: EventName,
	handler:
		| ProgressEventHandler
		| QueueEventHandler
		| OpenedAudioFilesHandler
		| ((event: { payload: ApplicationEvents[RuntimeEventName] }) => void),
): Promise<UnlistenFn> {
	if (event === EVENTS.PROGRESS) {
		return listenProcessingProgress(handler as ProgressEventHandler);
	}

	if (event === EVENTS.QUEUE) {
		return listenProcessingQueue(handler as QueueEventHandler);
	}

	if (event === EVENTS.OPENED_AUDIO_FILES) {
		return listenOpenedAudioFiles(handler as OpenedAudioFilesHandler);
	}

	return tauriListen(
		event,
		handler as (event: { payload: ApplicationEvents[RuntimeEventName] }) => void,
	);
}

function openDialog<T extends OpenDialogOptions>(options: T): Promise<OpenDialogReturn<T>>;
function openDialog(): Promise<string | string[] | null>;
function openDialog<T extends OpenDialogOptions>(
	options?: T,
): Promise<OpenDialogReturn<T> | string | string[] | null> {
	return tauriOpen(options) as Promise<OpenDialogReturn<T>>;
}

function openFile(options?: DialogOptions): Promise<string | null> {
	return tauriOpen({ ...options, multiple: false, directory: false } as const);
}

function openFiles(options?: DialogOptions): Promise<string[] | null> {
	return tauriOpen({ ...options, multiple: true, directory: false } as const);
}

function openDirectory(options?: DialogOptions): Promise<string | null> {
	return tauriOpen({ ...options, multiple: false, directory: true } as const);
}

export const tauriClient = {
	ping: (): Promise<CommandResult<'ping'>> => commandSpecs.ping(),
	echo: (input: string): Promise<CommandResult<'echo'>> => commandSpecs.echo({ input }),
	getAppSettings: (): Promise<AppSettings> => commandSpecs.get_app_settings(),
	updateAppSettings: (patch: AppSettingsPatch): Promise<AppSettings> =>
		commandSpecs.update_app_settings({ patch }),
	resetAppSettings: (): Promise<AppSettings> => commandSpecs.reset_app_settings(),
	validateFiles: (filePaths: string[]): Promise<CommandResult<'validate_files'>> =>
		commandSpecs.validate_files({ filePaths }),
	readAudioMetadata: (filePath: string): Promise<CommandResult<'read_audio_metadata'>> =>
		commandSpecs.read_audio_metadata({ filePath }),
	writeCoverArt: (
		filePath: string,
		coverData: number[],
	): Promise<CommandResult<'write_cover_art'>> =>
		commandSpecs.write_cover_art({ filePath, coverData }),
	loadCoverArtFile: (filePath: string): Promise<CommandResult<'load_cover_art_file'>> =>
		commandSpecs.load_cover_art_file({ filePath }),
	loadCoverArtFromUrl: (url: string): Promise<CommandResult<'load_cover_art_from_url'>> =>
		commandSpecs.load_cover_art_from_url({ url }),
	saveMetadataIntentToFile: (
		filePath: string,
		metadataIntent: MetadataIntentPatch,
	): Promise<CommandResult<'save_metadata_to_file'>> =>
		commandSpecs.save_metadata_to_file({ filePath, metadataIntent }),
	saveMetadataBatch: (
		items: MetadataSaveRequest[],
	): Promise<CommandResult<'save_metadata_batch'>> => commandSpecs.save_metadata_batch({ items }),
	searchOnlineMetadata: (args: {
		query: string;
		sources: MetadataSource[] | null;
		limit?: number | null;
	}): Promise<CommandResult<'search_online_metadata'>> => commandSpecs.search_online_metadata(args),
	analyzeAudioFiles: (filePaths: string[]): Promise<FileListInfo> =>
		commandSpecs.analyze_audio_files({ filePaths }),
	getSupportedAudioImportMetadata: (): Promise<
		CommandResult<'get_supported_audio_import_metadata'>
	> => commandSpecs.get_supported_audio_import_metadata(),
	discoverAudioImportPaths: (
		inputPaths: string[],
	): Promise<CommandResult<'discover_audio_import_paths'>> =>
		commandSpecs.discover_audio_import_paths({ inputPaths }),
	takeOpenedAudioFiles: (): Promise<CommandResult<'take_opened_audio_files'>> =>
		commandSpecs.take_opened_audio_files(),
	validateEncoderSettings: (
		settings: EncoderSettings,
		externalToolchain?: ExternalToolchainPreference | null,
	): Promise<CommandResult<'validate_encoder_settings'>> =>
		commandSpecs.validate_encoder_settings({
			settings,
			externalToolchain: externalToolchain ?? null,
		}),
	listAvailableEncoders: (
		externalToolchain?: ExternalToolchainPreference | null,
	): Promise<CommandResult<'list_available_encoders'>> =>
		commandSpecs.list_available_encoders({
			externalToolchain: externalToolchain ?? null,
		}),
	refreshExternalToolchain: (
		externalToolchain?: ExternalToolchainPreference | null,
	): Promise<CommandResult<'refresh_external_toolchain'>> =>
		commandSpecs.refresh_external_toolchain({
			externalToolchain: externalToolchain ?? null,
		}),
	getRuntimeSettingsCapabilities: (
		externalToolchain?: ExternalToolchainPreference | null,
	): Promise<RuntimeSettingsCapabilities> =>
		commandSpecs.get_runtime_settings_capabilities({
			externalToolchain: externalToolchain ?? null,
		}),
	previewOutputPath: (args: {
		outputDir: string;
		metadata?: Partial<AudiobookMetadata> | null;
		outputNaming?: ProcessPayload['outputNaming'] | null;
		sourcePath?: string | null;
		outputKind?: OutputKind | null;
	}): Promise<CommandResult<'preview_output_path'>> => commandSpecs.preview_output_path(args),
	preflightProcessingPlan: (args: {
		payload: ProcessPayload;
		metadataIntent?: MetadataIntentByPath | null;
		previewSeconds?: number | null;
	}): Promise<ProcessingPreflightPlan> => commandSpecs.preflight_processing_plan(args),
	getMaxConcurrentJobs: (): Promise<CommandResult<'get_max_concurrent_jobs'>> =>
		commandSpecs.get_max_concurrent_jobs(),
	setMaxConcurrentJobs: (
		maxConcurrent?: number | null,
	): Promise<CommandResult<'set_max_concurrent_jobs'>> =>
		commandSpecs.set_max_concurrent_jobs({ max_concurrent: maxConcurrent ?? null }),
	processAudiobookFiles: (args: {
		payload: ProcessPayload;
		metadataIntent?: MetadataIntentByPath | null;
		previewSeconds?: number | null;
	}): Promise<ProcessCommandResult> => commandSpecs.process_audiobook_files(args),
	cancelProcessing: (jobId?: string | null): Promise<CommandResult<'cancel_processing'>> =>
		jobId === undefined
			? commandSpecs.cancel_processing()
			: commandSpecs.cancel_processing({ job_id: jobId }),
	listen,
	open: openDialog,
	openFile,
	openFiles,
	openDirectory,
	openPath: (path: string, openWith?: string): Promise<void> => tauriOpenPath(path, openWith),
	openUrl: (url: string | URL, openWith?: string): Promise<void> => tauriOpenUrl(url, openWith),
};

export const TAURI_COMMAND_NAMES = Object.freeze(
	Object.keys(commandSpecs),
) as readonly TauriCommand[];

export const TAURI_APP_EVENT_NAMES = Object.freeze([
	'processing-progress',
	'processing-queue',
	'opened-audio-files',
] as const);

export type { TauriCommand };
