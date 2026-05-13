import {
	commands as generatedCommands,
	type OutputNamingConfig as GeneratedOutputNamingConfig,
} from '../generated/tauri';
import type {
	EncoderSettings,
	ExternalToolchainPreference,
	OutputKind,
	ProcessPayload,
	ProcessingPreflightPlan,
} from '../../types/audio';
import type { AudiobookMetadata, MetadataSaveRequest, MetadataSource } from '../../types/metadata';
import { compileMetadataIntentPatch, type MetadataIntentPatch } from '../../types/metadataIntent';
import { normalizeAppError, unwrapGeneratedResult } from './appError';
import {
	denormalizeMetadata,
	denormalizeNullish,
	denormalizeProcessPayload,
	normalizeEncoderAvailability,
	normalizeFileList,
	normalizeLookupResult,
	normalizeMetadata,
	normalizeMetadataSaveBatchResult,
	normalizeNullish,
	normalizeProcessResult,
} from './normalizers';

type MetadataIntentPayload = Record<string, MetadataIntentPatch>;
type GeneratedExternalToolchain = { overridePath: string | null };

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

function compileMetadataSaveRequests(items: MetadataSaveRequest[]): MetadataSaveRequest[] {
	return items.map((item) => ({
		filePath: item.filePath,
		metadataPatch: compileMetadataIntentPatch(item.metadataPatch),
	}));
}

export const commandSpecs = {
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
	save_metadata_batch: (args: { items: MetadataSaveRequest[] }) =>
		runGeneratedCommand(
			generatedCommands.saveMetadataBatch(compileMetadataSaveRequests(args.items)),
			normalizeMetadataSaveBatchResult,
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

export type TauriCommand = keyof typeof commandSpecs;
export type CommandResult<K extends TauriCommand> = Awaited<ReturnType<(typeof commandSpecs)[K]>>;
