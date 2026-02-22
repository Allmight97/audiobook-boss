import { listen as tauriListen, type UnlistenFn } from '@tauri-apps/api/event';
import { open as tauriOpen, type OpenDialogOptions } from '@tauri-apps/plugin-dialog';
import { openPath as tauriOpenExternal } from '@tauri-apps/plugin-opener';

import {
	commands as generatedCommands,
	events as generatedEvents,
	type MetadataSource as GeneratedMetadataSource,
	type OutputNamingConfig as GeneratedOutputNamingConfig,
} from '../generated/tauri';
import type { ApplicationEvents, EventName } from '../../types/events';
import type {
	EncoderSettings,
	ProcessV2Payload,
	FileListInfo,
	ProcessCommandResult,
} from '../../types/audio';
import type { AudiobookMetadata, MetadataSource } from '../../types/metadata';
import { compileMetadataIntentPatch, type MetadataIntentPatch } from '../../types/metadataIntent';
import {
	denormalizeMetadata,
	denormalizeNullish,
	denormalizeProcessPayload,
	normalizeFileList,
	normalizeLookupResult,
	normalizeMetadata,
	normalizeProcessResult,
	normalizeProgressEvent,
	normalizeQueueEvent,
} from './normalizers';

type MetadataPayload = Record<string, Partial<AudiobookMetadata>>;
type MetadataIntentPayload = Record<string, MetadataIntentPatch>;

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
		metadata?: Partial<AudiobookMetadata>;
		metadataIntent?: MetadataIntentPatch;
	}) => {
		const metadata = args.metadataIntent
			? compileMetadataIntentPatch(args.metadataIntent)
			: (args.metadata ?? {});
		return generatedCommands.saveMetadataToFile(args.filePath, denormalizeMetadata(metadata));
	},
	search_online_metadata: (args: {
		query: string;
		sources?: MetadataSource[] | null;
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
	preview_output_path: (args: {
		outputDir: string;
		metadata?: Partial<AudiobookMetadata> | null;
		outputNaming?: ProcessV2Payload['outputNaming'] | null;
		sourcePath?: string | null;
	}) =>
		generatedCommands.previewOutputPath(
			args.outputDir,
			args.metadata ? denormalizeMetadata(args.metadata) : null,
			args.outputNaming
				? (denormalizeNullish(args.outputNaming) as GeneratedOutputNamingConfig)
				: null,
			args.sourcePath ?? null,
		),
	get_max_concurrent_jobs: (_args?: undefined) => generatedCommands.getMaxConcurrentJobs(),
	set_max_concurrent_jobs: (args: { max_concurrent?: number | null }) =>
		generatedCommands.setMaxConcurrentJobs(args.max_concurrent ?? null),
	process_audiobook_files_v2: (args: {
		payload: ProcessV2Payload;
		metadata?: MetadataPayload | null;
		metadataIntent?: MetadataIntentPayload | null;
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

type TauriCommand = keyof typeof commandInvokers;
type CommandArgs<K extends TauriCommand> = Parameters<(typeof commandInvokers)[K]>[0];
type CommandResult<K extends TauriCommand> = Awaited<ReturnType<(typeof commandInvokers)[K]>>;

async function invokeCommand<K extends TauriCommand>(
	cmd: K,
	args?: CommandArgs<K>,
): Promise<CommandResult<K>> {
	const command = commandInvokers[cmd];
	return (await command(args as never)) as CommandResult<K>;
}

export const tauriClient = {
	ping: (): Promise<CommandResult<'ping'>> => invokeCommand('ping'),
	echo: (input: string): Promise<CommandResult<'echo'>> => invokeCommand('echo', { input }),
	validateFiles: (filePaths: string[]): Promise<CommandResult<'validate_files'>> =>
		invokeCommand('validate_files', { filePaths }),
	readAudioMetadata: (filePath: string): Promise<CommandResult<'read_audio_metadata'>> =>
		invokeCommand('read_audio_metadata', { filePath }),
	writeCoverArt: (
		filePath: string,
		coverData: number[],
	): Promise<CommandResult<'write_cover_art'>> =>
		invokeCommand('write_cover_art', { filePath, coverData }),
	loadCoverArtFile: (filePath: string): Promise<CommandResult<'load_cover_art_file'>> =>
		invokeCommand('load_cover_art_file', { filePath }),
	loadCoverArtFromUrl: (url: string): Promise<CommandResult<'load_cover_art_from_url'>> =>
		invokeCommand('load_cover_art_from_url', { url }),
	saveMetadataToFile: (
		filePath: string,
		metadata: Partial<AudiobookMetadata>,
	): Promise<CommandResult<'save_metadata_to_file'>> =>
		invokeCommand('save_metadata_to_file', { filePath, metadata }),
	saveMetadataIntentToFile: (
		filePath: string,
		metadataIntent: MetadataIntentPatch,
	): Promise<CommandResult<'save_metadata_to_file'>> =>
		invokeCommand('save_metadata_to_file', { filePath, metadataIntent }),
	searchOnlineMetadata: (args: {
		query: string;
		sources?: MetadataSource[] | null;
		limit?: number | null;
	}): Promise<CommandResult<'search_online_metadata'>> =>
		invokeCommand('search_online_metadata', args),
	analyzeAudioFiles: (filePaths: string[]): Promise<FileListInfo> =>
		invokeCommand('analyze_audio_files', { filePaths }),
	validateEncoderSettings: (
		settings: EncoderSettings,
	): Promise<CommandResult<'validate_encoder_settings_cmd'>> =>
		invokeCommand('validate_encoder_settings_cmd', { settings }),
	listAvailableEncoders: (): Promise<CommandResult<'list_available_encoders'>> =>
		invokeCommand('list_available_encoders'),
	previewOutputPath: (args: {
		outputDir: string;
		metadata?: Partial<AudiobookMetadata> | null;
		outputNaming?: ProcessV2Payload['outputNaming'] | null;
		sourcePath?: string | null;
	}): Promise<CommandResult<'preview_output_path'>> => invokeCommand('preview_output_path', args),
	getMaxConcurrentJobs: (): Promise<CommandResult<'get_max_concurrent_jobs'>> =>
		invokeCommand('get_max_concurrent_jobs'),
	setMaxConcurrentJobs: (
		maxConcurrent?: number | null,
	): Promise<CommandResult<'set_max_concurrent_jobs'>> =>
		invokeCommand('set_max_concurrent_jobs', { max_concurrent: maxConcurrent ?? null }),
	processAudiobookFilesV2: (args: {
		payload: ProcessV2Payload;
		metadata?: MetadataPayload | null;
		metadataIntent?: MetadataIntentPayload | null;
		previewSeconds?: number | null;
	}): Promise<ProcessCommandResult & { previewActualSeconds?: number; jobId: string }> =>
		invokeCommand('process_audiobook_files_v2', args),
	cancelProcessing: (jobId?: string | null): Promise<CommandResult<'cancel_processing'>> =>
		jobId === undefined
			? invokeCommand('cancel_processing')
			: invokeCommand('cancel_processing', { job_id: jobId }),
	listen: async <E extends EventName>(
		event: E,
		handler: (event: { payload: ApplicationEvents[E] }) => void,
	): Promise<UnlistenFn> => {
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
	},
	open: (options?: OpenDialogOptions): Promise<null | string | string[]> => tauriOpen(options),
	openExternal: (path: string): Promise<void> => tauriOpenExternal(path),
};

export const TAURI_COMMAND_NAMES = Object.freeze(
	Object.keys(commandInvokers),
) as readonly TauriCommand[];

export const TAURI_APP_EVENT_NAMES = Object.freeze([
	'processing-progress',
	'processing-queue',
] as const);

export type { TauriCommand };
