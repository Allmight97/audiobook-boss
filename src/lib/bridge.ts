/// <reference types="vite/client" />
// EXCEPTION: bridge boundary adapter intentionally exceeds 400 LOC while it centralizes
// IPC nullish normalization, metadata intent compilation, and dev/test runtime seams.
import { listen as tauriListen, type UnlistenFn } from '@tauri-apps/api/event';
import { open as tauriOpen, type OpenDialogOptions } from '@tauri-apps/plugin-dialog';
import { openPath as tauriOpenExternal } from '@tauri-apps/plugin-opener';

import {
	commands as generatedCommands,
	events as generatedEvents,
	type AudiobookMetadata as GeneratedAudiobookMetadata,
	type FileListInfo as GeneratedFileListInfo,
	type MetadataSource as GeneratedMetadataSource,
	type OnlineMetadataResult as GeneratedOnlineMetadataResult,
	type ProcessCommandResult as GeneratedProcessCommandResult,
	type ProcessV2Payload as GeneratedProcessV2Payload,
	type ProgressEvent as GeneratedProgressEvent,
	type QueueEvent as GeneratedQueueEvent,
} from './generated/tauri';
import type {
	ApplicationEvents,
	EventName,
	ProcessingProgressEvent,
	ProcessingQueueEvent,
} from '../types/events';
import type {
	EncoderSettings,
	FileListInfo as LegacyFileListInfo,
	ProcessCommandResult as LegacyProcessCommandResult,
	ProcessV2Payload as LegacyProcessV2Payload,
} from '../types/audio';
import type {
	AudiobookMetadata as LegacyAudiobookMetadata,
	MetadataSource as LegacyMetadataSource,
	OnlineMetadataResult as LegacyOnlineMetadataResult,
} from '../types/metadata';
import { compileMetadataIntentPatch, type MetadataIntentPatch } from '../types/metadataIntent';

// Check if we are running in a Tauri environment
const isTauri = !!window.__TAURI_INTERNALS__;
console.log(`[Bridge] Initialized. isTauri=${isTauri}, DEV=${import.meta.env.DEV}`);

type PlainRecord = Record<string, unknown>;

const METADATA_FIELDS = [
	'title',
	'artist',
	'album',
	'composer',
	'genre',
	'date',
	'track',
	'disk',
	'comment',
	'description',
	'series',
	'series_part',
	'subseries',
	'subseries_part',
	'album_sort',
	'cover_art',
] as const satisfies readonly (keyof GeneratedAudiobookMetadata)[];

const PROCESS_PAYLOAD_NULLABLE_FIELDS = [
	'sampleRate',
	'jobType',
	'outputNaming',
] as const satisfies readonly (keyof GeneratedProcessV2Payload)[];

const isPlainRecord = (value: unknown): value is PlainRecord =>
	typeof value === 'object' && value !== null && !Array.isArray(value);

function isScalarArrayWithoutNullish(value: readonly unknown[]): boolean {
	for (const entry of value) {
		if (entry == null || typeof entry === 'object') {
			return false;
		}
	}
	return true;
}

function normalizeNullish<T>(value: T): T {
	if (value == null) {
		return undefined as T;
	}
	if (Array.isArray(value)) {
		if (isScalarArrayWithoutNullish(value)) {
			return value as T;
		}
		const normalized = new Array<unknown>(value.length);
		for (const [index, entry] of value.entries()) {
			normalized[index] = normalizeNullish(entry);
		}
		return normalized as T;
	}
	if (isPlainRecord(value)) {
		const normalized: PlainRecord = {};
		for (const [key, entryValue] of Object.entries(value)) {
			const converted = normalizeNullish(entryValue);
			if (converted !== undefined) {
				normalized[key] = converted;
			}
		}
		return normalized as T;
	}
	return value;
}

function denormalizeNullish<T>(value: T): T {
	if (value === undefined) {
		return null as T;
	}
	if (Array.isArray(value)) {
		if (isScalarArrayWithoutNullish(value)) {
			return value as T;
		}
		const normalized = new Array<unknown>(value.length);
		for (const [index, entry] of value.entries()) {
			normalized[index] = denormalizeNullish(entry);
		}
		return normalized as T;
	}
	if (isPlainRecord(value)) {
		const normalized: PlainRecord = {};
		for (const [key, entryValue] of Object.entries(value)) {
			normalized[key] = denormalizeNullish(entryValue);
		}
		return normalized as T;
	}
	return value;
}

function toNullableShape<T extends PlainRecord, K extends keyof T>(
	input: Partial<Record<K, unknown>>,
	keys: readonly K[],
): Pick<T, K> {
	const output = {} as Pick<T, K>;
	for (const key of keys) {
		const value = input[key];
		(output as PlainRecord)[key as string] = value === undefined ? null : denormalizeNullish(value);
	}
	return output;
}

function normalizeMetadata(metadata: GeneratedAudiobookMetadata): LegacyAudiobookMetadata {
	return normalizeNullish(metadata) as LegacyAudiobookMetadata;
}

function denormalizeMetadata(
	metadata: Partial<LegacyAudiobookMetadata>,
): GeneratedAudiobookMetadata {
	return toNullableShape<GeneratedAudiobookMetadata, (typeof METADATA_FIELDS)[number]>(
		metadata as Partial<Record<(typeof METADATA_FIELDS)[number], unknown>>,
		METADATA_FIELDS,
	) as GeneratedAudiobookMetadata;
}

function normalizeFileList(info: GeneratedFileListInfo): LegacyFileListInfo {
	return normalizeNullish(info) as LegacyFileListInfo;
}

function normalizeLookupResult(result: GeneratedOnlineMetadataResult): LegacyOnlineMetadataResult {
	return normalizeNullish(result) as LegacyOnlineMetadataResult;
}

function denormalizeProcessPayload(payload: LegacyProcessV2Payload): GeneratedProcessV2Payload {
	const nullableFields = toNullableShape<
		GeneratedProcessV2Payload,
		(typeof PROCESS_PAYLOAD_NULLABLE_FIELDS)[number]
	>(
		payload as Partial<Record<(typeof PROCESS_PAYLOAD_NULLABLE_FIELDS)[number], unknown>>,
		PROCESS_PAYLOAD_NULLABLE_FIELDS,
	);
	return {
		inputFiles: payload.inputFiles,
		outputDir: payload.outputDir,
		settings: payload.settings as GeneratedProcessV2Payload['settings'],
		...nullableFields,
	};
}

function normalizeProcessResult(
	result: GeneratedProcessCommandResult,
): LegacyProcessCommandResult & { previewActualSeconds?: number; jobId: string } {
	return normalizeNullish(result) as LegacyProcessCommandResult & {
		previewActualSeconds?: number;
		jobId: string;
	};
}

function normalizeProgressEvent(payload: GeneratedProgressEvent): ProcessingProgressEvent {
	const normalized = normalizeNullish(payload) as ProcessingProgressEvent;
	return {
		...normalized,
		stage: payload.stage as ProcessingProgressEvent['stage'],
	};
}

function normalizeQueueEvent(payload: GeneratedQueueEvent): ProcessingQueueEvent {
	return normalizeNullish(payload) as ProcessingQueueEvent;
}

type LegacyMetadataPayload = Record<string, Partial<LegacyAudiobookMetadata>>;
type LegacyMetadataIntentPayload = Record<string, MetadataIntentPatch>;

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
		metadata?: Partial<LegacyAudiobookMetadata>;
		metadataIntent?: MetadataIntentPatch;
	}) => {
		const metadata = args.metadataIntent
			? compileMetadataIntentPatch(args.metadataIntent)
			: (args.metadata ?? {});
		return generatedCommands.saveMetadataToFile(args.filePath, denormalizeMetadata(metadata));
	},
	search_online_metadata: (args: {
		query: string;
		sources?: LegacyMetadataSource[] | null;
		limit?: number | null;
	}) =>
		generatedCommands
			.searchOnlineMetadata(
				args.query,
				(args.sources ?? null) as GeneratedMetadataSource[] | null,
				args.limit ?? null,
			)
			.then((results) => results.map(normalizeLookupResult)),
	analyze_audio_files: (args: { filePaths: string[] }) =>
		generatedCommands.analyzeAudioFiles(args.filePaths).then(normalizeFileList),
	validate_encoder_settings_cmd: (args: { settings: EncoderSettings }) =>
		generatedCommands.validateEncoderSettingsCmd(args.settings),
	list_available_encoders: (_args?: undefined) => generatedCommands.listAvailableEncoders(),
	get_max_concurrent_jobs: (_args?: undefined) => generatedCommands.getMaxConcurrentJobs(),
	set_max_concurrent_jobs: (args: { max_concurrent?: number | null }) =>
		generatedCommands.setMaxConcurrentJobs(args.max_concurrent ?? null),
	process_audiobook_files_v2: (args: {
		payload: LegacyProcessV2Payload;
		metadata?: LegacyMetadataPayload | null;
		metadataIntent?: LegacyMetadataIntentPayload | null;
		previewSeconds?: number | null;
	}) => {
		const metadataFromIntent = args.metadataIntent
			? Object.fromEntries(
					Object.entries(args.metadataIntent).map(([path, value]) => [
						path,
						compileMetadataIntentPatch(value),
					]),
				)
			: null;
		const metadataSource = metadataFromIntent ?? args.metadata ?? null;
		const metadataPayload = metadataSource
			? Object.fromEntries(
					Object.entries(metadataSource).map(([path, value]) => [path, denormalizeMetadata(value)]),
				)
			: null;

		return generatedCommands
			.processAudiobookFilesV2(
				denormalizeProcessPayload(args.payload),
				metadataPayload,
				args.previewSeconds ?? null,
			)
			.then(normalizeProcessResult);
	},
	cancel_processing: (args?: { job_id?: string | null }) =>
		generatedCommands.cancelProcessing(args?.job_id ?? null),
} as const;

type BridgeCommand = keyof typeof commandInvokers;
type BridgeCommandArgs<K extends BridgeCommand> = Parameters<(typeof commandInvokers)[K]>[0];
type BridgeCommandResult<K extends BridgeCommand> = Awaited<
	ReturnType<(typeof commandInvokers)[K]>
>;

// Helper to lazily load mocks only in DEV mode
async function getMocks() {
	// FALLBACK[FB-015]: trigger=runtime lacks Tauri bridge (web tests/dev shell)
	// observe=bridge init + warn logs for ignored non-tauri commands/listeners
	// sunset=2026-06-30 issue=#195
	console.log('[Bridge] Loading mocks...');
	if (import.meta.env.DEV) {
		return await import('./mocks');
	}
	throw new Error('Mocks are not available in production builds');
}

async function invokeCommand<K extends BridgeCommand>(
	cmd: K,
	args?: BridgeCommandArgs<K>,
): Promise<BridgeCommandResult<K>> {
	if (isTauri) {
		const command = commandInvokers[cmd];
		return (await command(args as never)) as unknown as BridgeCommandResult<K>;
	}

	if (import.meta.env.DEV) {
		const mocks = await getMocks();
		return (await mocks.mockInvoke(cmd, args as never)) as unknown as BridgeCommandResult<K>;
	}

	console.warn(`[Bridge] Tauri not detected and not in DEV mode. Command '${cmd}' ignored.`);
	return Promise.reject('Tauri API not available') as unknown as BridgeCommandResult<K>;
}

export const bridge = {
	ping: (): Promise<BridgeCommandResult<'ping'>> => invokeCommand('ping'),
	echo: (input: string): Promise<BridgeCommandResult<'echo'>> => invokeCommand('echo', { input }),
	validateFiles: (filePaths: string[]): Promise<BridgeCommandResult<'validate_files'>> =>
		invokeCommand('validate_files', { filePaths }),
	readAudioMetadata: (filePath: string): Promise<BridgeCommandResult<'read_audio_metadata'>> =>
		invokeCommand('read_audio_metadata', { filePath }),
	writeCoverArt: (
		filePath: string,
		coverData: number[],
	): Promise<BridgeCommandResult<'write_cover_art'>> =>
		invokeCommand('write_cover_art', { filePath, coverData }),
	loadCoverArtFile: (filePath: string): Promise<BridgeCommandResult<'load_cover_art_file'>> =>
		invokeCommand('load_cover_art_file', { filePath }),
	loadCoverArtFromUrl: (url: string): Promise<BridgeCommandResult<'load_cover_art_from_url'>> =>
		invokeCommand('load_cover_art_from_url', { url }),
	saveMetadataToFile: (
		filePath: string,
		metadata: Partial<LegacyAudiobookMetadata>,
	): Promise<BridgeCommandResult<'save_metadata_to_file'>> =>
		invokeCommand('save_metadata_to_file', { filePath, metadata }),
	saveMetadataIntentToFile: (
		filePath: string,
		metadataIntent: MetadataIntentPatch,
	): Promise<BridgeCommandResult<'save_metadata_to_file'>> =>
		invokeCommand('save_metadata_to_file', { filePath, metadataIntent }),
	searchOnlineMetadata: (args: {
		query: string;
		sources?: LegacyMetadataSource[] | null;
		limit?: number | null;
	}): Promise<BridgeCommandResult<'search_online_metadata'>> =>
		invokeCommand('search_online_metadata', args),
	analyzeAudioFiles: (filePaths: string[]): Promise<BridgeCommandResult<'analyze_audio_files'>> =>
		invokeCommand('analyze_audio_files', { filePaths }),
	validateEncoderSettings: (
		settings: EncoderSettings,
	): Promise<BridgeCommandResult<'validate_encoder_settings_cmd'>> =>
		invokeCommand('validate_encoder_settings_cmd', { settings }),
	listAvailableEncoders: (): Promise<BridgeCommandResult<'list_available_encoders'>> =>
		invokeCommand('list_available_encoders'),
	getMaxConcurrentJobs: (): Promise<BridgeCommandResult<'get_max_concurrent_jobs'>> =>
		invokeCommand('get_max_concurrent_jobs'),
	setMaxConcurrentJobs: (
		maxConcurrent?: number | null,
	): Promise<BridgeCommandResult<'set_max_concurrent_jobs'>> =>
		invokeCommand('set_max_concurrent_jobs', { max_concurrent: maxConcurrent ?? null }),
	processAudiobookFilesV2: (args: {
		payload: LegacyProcessV2Payload;
		metadata?: LegacyMetadataPayload | null;
		metadataIntent?: LegacyMetadataIntentPayload | null;
		previewSeconds?: number | null;
	}): Promise<BridgeCommandResult<'process_audiobook_files_v2'>> =>
		invokeCommand('process_audiobook_files_v2', args),
	cancelProcessing: (jobId?: string | null): Promise<BridgeCommandResult<'cancel_processing'>> =>
		jobId === undefined
			? invokeCommand('cancel_processing')
			: invokeCommand('cancel_processing', { job_id: jobId }),

	/**
	 * Typed wrapper for Tauri listen() with generated app events + built-in Tauri events.
	 */
	listen: async <E extends EventName>(
		event: E,
		handler: (event: { payload: ApplicationEvents[E] }) => void,
	): Promise<UnlistenFn> => {
		if (isTauri) {
			if (event === 'processing-progress') {
				return generatedEvents.processingProgress.listen((evt) => {
					handler({ payload: normalizeProgressEvent(evt.payload) as ApplicationEvents[E] });
				});
			}

			if (event === 'processing-queue') {
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

		console.warn('[Bridge] Tauri not detected. Dialog open ignored.');
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
	Object.keys(commandInvokers),
) as readonly BridgeCommand[];

export const BRIDGE_APP_EVENT_NAMES = Object.freeze([
	'processing-progress',
	'processing-queue',
] as const);

export type { BridgeCommand };
