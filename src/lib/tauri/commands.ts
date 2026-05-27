import {
	commands as generatedCommands,
	type AppSettingsPatch as GeneratedAppSettingsPatch,
	type EncoderDefaults as GeneratedEncoderDefaults,
	type ExternalToolchainPreference as GeneratedExternalToolchainPreference,
	type OutputDefaults as GeneratedOutputDefaults,
	type OutputNamingConfig as GeneratedOutputNamingConfig,
} from '../generated/tauri';
import type {
	EncoderSettings,
	ExternalToolchainPreference,
	OutputNamingConfig,
	OutputKind,
	ProcessPayload,
	ProcessingPreflightPlan,
	RuntimeSettingsCapabilities,
} from '../../types/audio';
import type { AppSettings, AppSettingsPatch } from '../../types/appSettings';
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
	normalizeRuntimeSettingsCapabilities,
} from './normalizers';

type MetadataIntentByPath = Record<string, MetadataIntentPatch>;

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
): GeneratedExternalToolchainPreference | null {
	return externalToolchain
		? {
				overridePath: externalToolchain.overridePath ?? null,
			}
		: null;
}

function toGeneratedRequiredOutputNamingConfig(
	outputNaming: OutputNamingConfig,
): GeneratedOutputNamingConfig {
	const denormalized = denormalizeNullish(outputNaming);
	return {
		preset: denormalized.preset,
		includeYear: denormalized.includeYear,
		customTemplate: denormalized.customTemplate ?? null,
	};
}

function toGeneratedOutputNamingConfig(
	outputNaming?: ProcessPayload['outputNaming'] | null,
): GeneratedOutputNamingConfig | null {
	return outputNaming ? toGeneratedRequiredOutputNamingConfig(outputNaming) : null;
}

function toGeneratedEncoderDefaults(
	defaults: AppSettings['encoderDefaults'],
): GeneratedEncoderDefaults {
	return {
		settings: defaults.settings,
		sampleRate: defaults.sampleRate,
		externalToolchain: toGeneratedExternalToolchain(defaults.externalToolchain) ?? {
			overridePath: null,
		},
	};
}

function toGeneratedOutputDefaults(
	defaults: AppSettings['outputDefaults'],
): GeneratedOutputDefaults {
	return {
		outputDirectory: defaults.outputDirectory ?? null,
		outputNaming: toGeneratedRequiredOutputNamingConfig(defaults.outputNaming),
	};
}

function toGeneratedAppSettingsPatch(patch: AppSettingsPatch): GeneratedAppSettingsPatch {
	return {
		maxConcurrentJobs: patch.maxConcurrentJobs ?? null,
		encoderDefaults: patch.encoderDefaults
			? toGeneratedEncoderDefaults(patch.encoderDefaults)
			: null,
		outputDefaults: patch.outputDefaults ? toGeneratedOutputDefaults(patch.outputDefaults) : null,
	};
}

function compileMetadataIntentMap(
	metadataIntentByPath?: MetadataIntentByPath | null,
): Record<string, MetadataIntentPatch> | null {
	if (!metadataIntentByPath) {
		return null;
	}

	return Object.fromEntries(
		Object.entries(metadataIntentByPath).map(([path, value]) => [
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
	get_app_settings: (_args?: undefined) =>
		runGeneratedCommand(generatedCommands.getAppSettings(), (settings) =>
			normalizeNullish(settings),
		),
	update_app_settings: (args: { patch: AppSettingsPatch }) =>
		runGeneratedCommand(
			generatedCommands.updateAppSettings(toGeneratedAppSettingsPatch(args.patch)),
			(settings) => normalizeNullish(settings),
		),
	reset_app_settings: (_args?: undefined) =>
		runGeneratedCommand(generatedCommands.resetAppSettings(), (settings) =>
			normalizeNullish(settings),
		),
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
	get_supported_audio_import_metadata: (_args?: undefined) =>
		runGeneratedCommand(generatedCommands.getSupportedAudioImportMetadata()),
	discover_audio_import_paths: (args: { inputPaths: string[] }) =>
		runGeneratedCommand(generatedCommands.discoverAudioImportPaths(args.inputPaths)),
	take_opened_audio_files: (_args?: undefined) =>
		runGeneratedCommand(generatedCommands.takeOpenedAudioFiles()),
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
	get_runtime_settings_capabilities: (args: {
		externalToolchain: ExternalToolchainPreference | null;
	}): Promise<RuntimeSettingsCapabilities> =>
		runGeneratedCommand(
			generatedCommands.getRuntimeSettingsCapabilities(
				toGeneratedExternalToolchain(args.externalToolchain),
			),
			normalizeRuntimeSettingsCapabilities,
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
		metadataIntent?: MetadataIntentByPath | null;
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
		metadataIntent?: MetadataIntentByPath | null;
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
