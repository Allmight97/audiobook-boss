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
	type WorkOperationListSnapshotEvent,
	type WorkOperationSnapshotEvent,
} from '../../types/events';
import type {
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
import type {
	MetadataIntentPatch,
	MetadataIntentValidationResult,
} from '../../types/metadataIntent';
import type {
	AcquisitionJob,
	AcquisitionPlan,
	ProviderId,
	RemoteAuthCompletionRequest,
	RemoteAuthStartResponse,
	RemoteLibraryResponse,
	RemoteSourceAccountState,
	RemoteSourceProviderCapabilities,
} from '../../types/remoteSource';
import type {
	OperationId,
	OperationListSnapshot,
	OperationSnapshot,
	SubmitProcessingOperationRequest,
	WorkSubmissionAccepted,
} from '../../types/workRuntime';
import { commandSpecs, type CommandResult, type TauriCommand } from './commands';
import {
	normalizeOperationListSnapshot,
	normalizeOperationSnapshot,
	normalizeProgressEvent,
	normalizeQueueEvent,
} from './normalizers';

type MetadataIntentByPath = Record<string, MetadataIntentPatch>;
type AppEventName = (typeof TAURI_APP_EVENT_NAMES)[number];
type RuntimeEventName = Exclude<EventName, AppEventName>;
type ProgressEventHandler = (event: { payload: ProcessingProgressEvent }) => void;
type QueueEventHandler = (event: { payload: ProcessingQueueEvent }) => void;
type OpenedAudioFilesHandler = (event: { payload: OpenedAudioFilesEvent }) => void;
type WorkOperationSnapshotHandler = (event: { payload: WorkOperationSnapshotEvent }) => void;
type WorkOperationListSnapshotHandler = (event: {
	payload: WorkOperationListSnapshotEvent;
}) => void;
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

async function listenWorkOperationSnapshot(
	handler: WorkOperationSnapshotHandler,
): Promise<UnlistenFn> {
	return generatedEvents.workOperationSnapshot.listen((event) => {
		handler({ payload: { snapshot: normalizeOperationSnapshot(event.payload.snapshot) } });
	});
}

async function listenWorkOperationListSnapshot(
	handler: WorkOperationListSnapshotHandler,
): Promise<UnlistenFn> {
	return generatedEvents.workOperationListSnapshot.listen((event) => {
		handler({
			payload: normalizeOperationListSnapshot({ operations: event.payload.operations }),
		});
	});
}

function listen(event: typeof EVENTS.PROGRESS, handler: ProgressEventHandler): Promise<UnlistenFn>;
function listen(event: typeof EVENTS.QUEUE, handler: QueueEventHandler): Promise<UnlistenFn>;
function listen(
	event: typeof EVENTS.OPENED_AUDIO_FILES,
	handler: OpenedAudioFilesHandler,
): Promise<UnlistenFn>;
function listen(
	event: typeof EVENTS.WORK_OPERATION_SNAPSHOT,
	handler: WorkOperationSnapshotHandler,
): Promise<UnlistenFn>;
function listen(
	event: typeof EVENTS.WORK_OPERATION_LIST_SNAPSHOT,
	handler: WorkOperationListSnapshotHandler,
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
		| WorkOperationSnapshotHandler
		| WorkOperationListSnapshotHandler
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

	if (event === EVENTS.WORK_OPERATION_SNAPSHOT) {
		return listenWorkOperationSnapshot(handler as WorkOperationSnapshotHandler);
	}

	if (event === EVENTS.WORK_OPERATION_LIST_SNAPSHOT) {
		return listenWorkOperationListSnapshot(handler as WorkOperationListSnapshotHandler);
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
	getAppSettings: (): Promise<AppSettings> => commandSpecs.get_app_settings(),
	updateAppSettings: (patch: AppSettingsPatch): Promise<AppSettings> =>
		commandSpecs.update_app_settings({ patch }),
	resetAppSettings: (): Promise<AppSettings> => commandSpecs.reset_app_settings(),
	validateFiles: (filePaths: string[]): Promise<CommandResult<'validate_files'>> =>
		commandSpecs.validate_files({ filePaths }),
	readAudioMetadata: (filePath: string): Promise<CommandResult<'read_audio_metadata'>> =>
		commandSpecs.read_audio_metadata({ filePath }),
	readAudioCoverThumbnail: (
		filePath: string,
	): Promise<CommandResult<'read_audio_cover_thumbnail'>> =>
		commandSpecs.read_audio_cover_thumbnail({ filePath }),
	writeCoverArt: (
		filePath: string,
		coverData: number[],
	): Promise<CommandResult<'write_cover_art'>> =>
		commandSpecs.write_cover_art({ filePath, coverData }),
	loadCoverArtFile: (filePath: string): Promise<CommandResult<'load_cover_art_file'>> =>
		commandSpecs.load_cover_art_file({ filePath }),
	loadCoverArtFromUrl: (url: string): Promise<CommandResult<'load_cover_art_from_url'>> =>
		commandSpecs.load_cover_art_from_url({ url }),
	validateMetadataIntentPatch: (
		metadataIntent: MetadataIntentPatch,
	): Promise<MetadataIntentValidationResult> =>
		commandSpecs.validate_metadata_intent_patch({ metadataIntent }),
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
	listRemoteSourceProviders: (): Promise<RemoteSourceProviderCapabilities[]> =>
		commandSpecs.list_remote_source_providers(),
	getRemoteSourceAccountState: (providerId: ProviderId): Promise<RemoteSourceAccountState> =>
		commandSpecs.get_remote_source_account_state({ providerId }),
	startRemoteSourceAuth: (providerId: ProviderId): Promise<RemoteAuthStartResponse> =>
		commandSpecs.start_remote_source_auth({ providerId }),
	completeRemoteSourceAuth: (
		request: RemoteAuthCompletionRequest,
	): Promise<RemoteSourceAccountState> => commandSpecs.complete_remote_source_auth({ request }),
	logoutRemoteSourceAccount: (providerId: ProviderId): Promise<RemoteSourceAccountState> =>
		commandSpecs.logout_remote_source_account({ providerId }),
	loadRemoteSourceLibrary: (providerId: ProviderId): Promise<RemoteLibraryResponse> =>
		commandSpecs.load_remote_source_library({ providerId }),
	startRemoteSourceAcquisition: (plan: AcquisitionPlan): Promise<AcquisitionJob> =>
		commandSpecs.start_remote_source_acquisition({ plan }),
	getRemoteSourceAcquisitionStatus: (jobId: string): Promise<AcquisitionJob> =>
		commandSpecs.get_remote_source_acquisition_status({ jobId }),
	cancelRemoteSourceAcquisition: (jobId: string): Promise<AcquisitionJob> =>
		commandSpecs.cancel_remote_source_acquisition({ jobId }),
	purgeRemoteSourceSession: (jobId: string): Promise<void> =>
		commandSpecs.purge_remote_source_session({ jobId }).then(() => undefined),
	validateEncoderSettings: (
		settings: EncoderSettings,
	): Promise<CommandResult<'validate_encoder_settings'>> =>
		commandSpecs.validate_encoder_settings({
			settings,
		}),
	getRuntimeSettingsCapabilities: (): Promise<RuntimeSettingsCapabilities> =>
		commandSpecs.get_runtime_settings_capabilities(),
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
	submitProcessingOperation: (
		args: SubmitProcessingOperationRequest,
	): Promise<WorkSubmissionAccepted> => commandSpecs.submit_processing_operation(args),
	listWorkOperations: (): Promise<OperationListSnapshot> => commandSpecs.list_work_operations(),
	getWorkOperation: (operationId: OperationId): Promise<OperationSnapshot> =>
		commandSpecs.get_work_operation({ operationId }),
	cancelWorkOperation: (operationId: OperationId): Promise<OperationSnapshot> =>
		commandSpecs.cancel_work_operation({ operationId }),
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
	'work-operation-snapshot',
	'work-operation-list-snapshot',
] as const);

export type { TauriCommand };
