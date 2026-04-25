import { listen as tauriListen, type UnlistenFn } from '@tauri-apps/api/event';
import { open as tauriOpen, type OpenDialogOptions } from '@tauri-apps/plugin-dialog';
import { openPath as tauriOpenExternal } from '@tauri-apps/plugin-opener';

import {
	commands as generatedCommands,
	events as generatedEvents,
	type OutputNamingConfig as GeneratedOutputNamingConfig,
} from '../generated/tauri';
import {
	EVENTS,
	type ApplicationEvents,
	type EventName,
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
} from '../../types/audio';
import type { AudiobookMetadata, MetadataSource } from '../../types/metadata';
import { compileMetadataIntentPatch, type MetadataIntentPatch } from '../../types/metadataIntent';
import { normalizeAppError, unwrapGeneratedResult } from './appError';
import {
	denormalizeMetadata,
	denormalizeNullish,
	denormalizeProcessPayload,
	normalizeNullish,
	normalizeEncoderAvailability,
	normalizeFileList,
	normalizeLookupResult,
	normalizeMetadata,
	normalizeProcessResult,
	normalizeProgressEvent,
	normalizeQueueEvent,
} from './normalizers';

type MetadataIntentPayload = Record<string, MetadataIntentPatch>;
type GeneratedExternalToolchain = { overridePath: string | null };
type AppEventName = (typeof TAURI_APP_EVENT_NAMES)[number];
type RuntimeEventName = Exclude<EventName, AppEventName>;
type ProgressEventHandler = (event: { payload: ProcessingProgressEvent }) => void;
type QueueEventHandler = (event: { payload: ProcessingQueueEvent }) => void;

type UnwrapGeneratedResult<T> = T extends { status: 'error' }
	? never
	: T extends { status: 'ok'; data: infer U }
		? U
		: T;

async function runGeneratedCommand<T>(promise: Promise<T>): Promise<UnwrapGeneratedResult<T>>;
async function runGeneratedCommand<T, R>(
	promise: Promise<T>,
	transform: (value: UnwrapGeneratedResult<T>) => R,
): Promise<R>;
async function runGeneratedCommand<T, R>(
	promise: Promise<T>,
	transform?: (value: UnwrapGeneratedResult<T>) => R,
): Promise<UnwrapGeneratedResult<T> | R> {
	try {
		const response = await promise;
		const unwrapped = unwrapGeneratedResult(response) as UnwrapGeneratedResult<T>;
		return transform ? transform(unwrapped) : unwrapped;
	} catch (error) {
		throw normalizeAppError(error);
	}
}

function toGeneratedExternalToolchain(
	externalToolchain?: ExternalToolchainPreference | null,
): GeneratedExternalToolchain | null {
	return externalToolchain
		? {
				overridePath: externalToolchain.overridePath ?? null,
			}
		: null;
}

function toGeneratedOutputNamingConfig(
	outputNaming?: ProcessPayload['outputNaming'] | null,
): GeneratedOutputNamingConfig | null {
	if (!outputNaming) {
		return null;
	}

	const denormalized = denormalizeNullish(outputNaming);
	return {
		preset: denormalized.preset,
		includeYear: denormalized.includeYear,
		customTemplate: denormalized.customTemplate ?? null,
	};
}

function compileMetadataIntentMap(
	metadataIntent?: MetadataIntentPayload | null,
): Record<string, MetadataIntentPatch> | null {
	if (!metadataIntent) {
		return null;
	}

	return Object.fromEntries(
		Object.entries(metadataIntent).map(([path, value]) => [
			path,
			compileMetadataIntentPatch(value),
		]),
	);
}

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

const commandSpecs = {
	ping: (_args?: undefined) => runGeneratedCommand(generatedCommands.ping()),
	echo: (args: { input: string }) => runGeneratedCommand(generatedCommands.echo(args.input)),
	validate_files: (args: { filePaths: string[] }) =>
		runGeneratedCommand(generatedCommands.validateFiles(args.filePaths)),
	read_audio_metadata: (args: { filePath: string }) =>
		runGeneratedCommand(generatedCommands.readAudioMetadata(args.filePath), normalizeMetadata),
	write_cover_art: (args: { filePath: string; coverData: number[] }) =>
		runGeneratedCommand(generatedCommands.writeCoverArt(args.filePath, args.coverData)),
	load_cover_art_file: (args: { filePath: string }) =>
		runGeneratedCommand(generatedCommands.loadCoverArtFile(args.filePath)),
	load_cover_art_from_url: (args: { url: string }) =>
		runGeneratedCommand(generatedCommands.loadCoverArtFromUrl(args.url)),
	save_metadata_to_file: (args: { filePath: string; metadataIntent: MetadataIntentPatch }) =>
		runGeneratedCommand(
			generatedCommands.saveMetadataToFile(
				args.filePath,
				compileMetadataIntentPatch(args.metadataIntent),
			),
		),
	search_online_metadata: (args: {
		query: string;
		sources: MetadataSource[] | null;
		limit?: number | null;
	}) =>
		runGeneratedCommand(
			generatedCommands.searchOnlineMetadata(args.query, args.sources, args.limit ?? null),
			(results) => results.map(normalizeLookupResult),
		),
	analyze_audio_files: (args: { filePaths: string[] }) =>
		runGeneratedCommand(generatedCommands.analyzeAudioFiles(args.filePaths), normalizeFileList),
	validate_encoder_settings: (args: {
		settings: EncoderSettings;
		externalToolchain?: ExternalToolchainPreference | null;
	}) =>
		runGeneratedCommand(
			generatedCommands.validateEncoderSettings(
				args.settings,
				toGeneratedExternalToolchain(args.externalToolchain),
			),
		),
	list_available_encoders: (args?: { externalToolchain?: ExternalToolchainPreference | null }) =>
		runGeneratedCommand(
			generatedCommands.listAvailableEncoders(
				toGeneratedExternalToolchain(args?.externalToolchain),
			),
			normalizeEncoderAvailability,
		),
	refresh_external_toolchain: (args?: { externalToolchain?: ExternalToolchainPreference | null }) =>
		runGeneratedCommand(
			generatedCommands.refreshExternalToolchain(
				toGeneratedExternalToolchain(args?.externalToolchain),
			),
			normalizeEncoderAvailability,
		),
	preview_output_path: (args: {
		outputDir: string;
		metadata?: Partial<AudiobookMetadata> | null;
		outputNaming?: ProcessPayload['outputNaming'] | null;
		sourcePath?: string | null;
		outputKind?: OutputKind | null;
	}) =>
		runGeneratedCommand(
			generatedCommands.previewOutputPath(
				args.outputDir,
				args.metadata ? denormalizeMetadata(args.metadata) : null,
				toGeneratedOutputNamingConfig(args.outputNaming),
				args.sourcePath ?? null,
				args.outputKind ?? null,
			),
		),
	preflight_processing_plan: (args: {
		payload: ProcessPayload;
		metadataIntent?: MetadataIntentPayload | null;
		previewSeconds?: number | null;
	}) =>
		runGeneratedCommand(
			generatedCommands.preflightProcessingPlan(
				denormalizeProcessPayload(args.payload),
				compileMetadataIntentMap(args.metadataIntent),
				args.previewSeconds ?? null,
			),
			(plan) => normalizeNullish(plan) as ProcessingPreflightPlan,
		),
	get_max_concurrent_jobs: (_args?: undefined) =>
		runGeneratedCommand(generatedCommands.getMaxConcurrentJobs()),
	set_max_concurrent_jobs: (args: { max_concurrent?: number | null }) =>
		runGeneratedCommand(generatedCommands.setMaxConcurrentJobs(args.max_concurrent ?? null)),
	process_audiobook_files: (args: {
		payload: ProcessPayload;
		metadataIntent?: MetadataIntentPayload | null;
		previewSeconds?: number | null;
	}) =>
		runGeneratedCommand(
			generatedCommands.processAudiobookFiles(
				denormalizeProcessPayload(args.payload),
				compileMetadataIntentMap(args.metadataIntent),
				args.previewSeconds ?? null,
			),
			normalizeProcessResult,
		),
	cancel_processing: (args?: { job_id?: string | null }) =>
		runGeneratedCommand(generatedCommands.cancelProcessing(args?.job_id ?? null)),
} as const;

type TauriCommand = keyof typeof commandSpecs;
type CommandResult<K extends TauriCommand> = Awaited<ReturnType<(typeof commandSpecs)[K]>>;

function listen(event: typeof EVENTS.PROGRESS, handler: ProgressEventHandler): Promise<UnlistenFn>;
function listen(event: typeof EVENTS.QUEUE, handler: QueueEventHandler): Promise<UnlistenFn>;
function listen<E extends RuntimeEventName>(
	event: E,
	handler: (event: { payload: ApplicationEvents[E] }) => void,
): Promise<UnlistenFn>;
function listen(
	event: EventName,
	handler:
		| ProgressEventHandler
		| QueueEventHandler
		| ((event: { payload: ApplicationEvents[RuntimeEventName] }) => void),
): Promise<UnlistenFn> {
	if (event === EVENTS.PROGRESS) {
		return listenProcessingProgress(handler as ProgressEventHandler);
	}

	if (event === EVENTS.QUEUE) {
		return listenProcessingQueue(handler as QueueEventHandler);
	}

	return tauriListen(
		event,
		handler as (event: { payload: ApplicationEvents[RuntimeEventName] }) => void,
	);
}

export const tauriClient = {
	ping: (): Promise<CommandResult<'ping'>> => commandSpecs.ping(),
	echo: (input: string): Promise<CommandResult<'echo'>> => commandSpecs.echo({ input }),
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
	searchOnlineMetadata: (args: {
		query: string;
		sources: MetadataSource[] | null;
		limit?: number | null;
	}): Promise<CommandResult<'search_online_metadata'>> => commandSpecs.search_online_metadata(args),
	analyzeAudioFiles: (filePaths: string[]): Promise<FileListInfo> =>
		commandSpecs.analyze_audio_files({ filePaths }),
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
	previewOutputPath: (args: {
		outputDir: string;
		metadata?: Partial<AudiobookMetadata> | null;
		outputNaming?: ProcessPayload['outputNaming'] | null;
		sourcePath?: string | null;
		outputKind?: OutputKind | null;
	}): Promise<CommandResult<'preview_output_path'>> => commandSpecs.preview_output_path(args),
	preflightProcessingPlan: (args: {
		payload: ProcessPayload;
		metadataIntent?: MetadataIntentPayload | null;
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
		metadataIntent?: MetadataIntentPayload | null;
		previewSeconds?: number | null;
	}): Promise<ProcessCommandResult> => commandSpecs.process_audiobook_files(args),
	cancelProcessing: (jobId?: string | null): Promise<CommandResult<'cancel_processing'>> =>
		jobId === undefined
			? commandSpecs.cancel_processing()
			: commandSpecs.cancel_processing({ job_id: jobId }),
	listen,
	open: (options?: OpenDialogOptions): Promise<null | string | string[]> => tauriOpen(options),
	openExternal: (path: string): Promise<void> => tauriOpenExternal(path),
};

export const TAURI_COMMAND_NAMES = Object.freeze(
	Object.keys(commandSpecs),
) as readonly TauriCommand[];

export const TAURI_APP_EVENT_NAMES = Object.freeze([
	'processing-progress',
	'processing-queue',
] as const);

export type { TauriCommand };
