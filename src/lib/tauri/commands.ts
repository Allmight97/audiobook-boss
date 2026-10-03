import {
	commands as generatedCommands,
	type EncoderDefaults as GeneratedEncoderDefaults,
	type OutputDefaults as GeneratedOutputDefaults,
	type SettingsIntent as GeneratedSettingsIntent,
	type OutputNamingConfig as GeneratedOutputNamingConfig,
	type RemoteAuthCompletionRequest as GeneratedRemoteAuthCompletionRequest,
} from '../generated/tauri';
import type { OutputNamingConfig } from '../../types/audio';
import type { AppSettings, SettingsIntent } from '../../types/appSettings';
import type { FrontendLogEntry } from '../../types/frontendLog';
import type { SessionIntent } from '../../types/session';
import type { ProviderId, RemoteAuthCompletionRequest } from '../../types/remoteSource';
import type { OperationId } from '../../types/workRuntime';
import { normalizeAppError, unwrapGeneratedResult } from './appError';
import {
	denormalizeNullish,
	normalizeFrontendAttachment,
	normalizeMetadata,
	normalizeNullish,
	normalizeOperationListSnapshot,
	normalizeOperationSnapshot,
	normalizeSessionReply,
	normalizeSettingsReply,
} from './normalizers';

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

function toGeneratedEncoderDefaults(
	defaults: AppSettings['encoderDefaults'],
): GeneratedEncoderDefaults {
	return {
		settings: defaults.settings,
		sampleRate: defaults.sampleRate,
		format: defaults.format,
		intent: defaults.intent,
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

function toGeneratedSettingsIntent(intent: SettingsIntent): GeneratedSettingsIntent {
	if (intent.kind !== 'remember') return intent;
	return {
		kind: 'remember',
		encoderDefaults: intent.encoderDefaults
			? toGeneratedEncoderDefaults(intent.encoderDefaults)
			: null,
		outputDefaults: intent.outputDefaults ? toGeneratedOutputDefaults(intent.outputDefaults) : null,
		defaultAcquisitionLane: intent.defaultAcquisitionLane ?? null,
	};
}

export const commandSpecs = {
	attach_frontend: (_args?: undefined) =>
		runGeneratedCommand(generatedCommands.attachFrontend(), normalizeFrontendAttachment),
	session_dispatch: (args: { client: number; sequence: number; intent: SessionIntent }) =>
		runGeneratedCommand(
			generatedCommands.sessionDispatch(args.client, args.sequence, args.intent),
			normalizeSessionReply,
		),
	settings_dispatch: (args: { client: number; sequence: number; intent: SettingsIntent }) =>
		runGeneratedCommand(
			generatedCommands.settingsDispatch(
				args.client,
				args.sequence,
				toGeneratedSettingsIntent(args.intent),
			),
			normalizeSettingsReply,
		),
	session_cover_art: (_args?: undefined) =>
		runGeneratedCommand(generatedCommands.sessionCoverArt()),
	read_audio_metadata: (args: { filePath: string }) =>
		runGeneratedCommand(generatedCommands.readAudioMetadata(args.filePath), normalizeMetadata),
	load_cover_art_from_url: (args: { url: string }) =>
		runGeneratedCommand(generatedCommands.loadCoverArtFromUrl(args.url)),
	read_audio_cover_thumbnail: (args: { filePath: string }) =>
		runGeneratedCommand(generatedCommands.readAudioCoverThumbnail(args.filePath)),
	get_supported_audio_import_metadata: (_args?: undefined) =>
		runGeneratedCommand(generatedCommands.getSupportedAudioImportMetadata()),
	list_remote_source_providers: (_args?: undefined) =>
		runGeneratedCommand(generatedCommands.listRemoteSourceProviders(), normalizeNullish),
	get_remote_source_account_state: (args: { providerId: ProviderId }) =>
		runGeneratedCommand(
			generatedCommands.getRemoteSourceAccountState(args.providerId),
			normalizeNullish,
		),
	start_remote_source_auth: (args: { providerId: ProviderId }) =>
		runGeneratedCommand(generatedCommands.startRemoteSourceAuth(args.providerId), normalizeNullish),
	complete_remote_source_auth: (args: { request: RemoteAuthCompletionRequest }) =>
		runGeneratedCommand(
			generatedCommands.completeRemoteSourceAuth(
				denormalizeNullish(args.request) as GeneratedRemoteAuthCompletionRequest,
			),
			normalizeNullish,
		),
	logout_remote_source_account: (args: { providerId: ProviderId }) =>
		runGeneratedCommand(
			generatedCommands.logoutRemoteSourceAccount(args.providerId),
			normalizeNullish,
		),
	load_remote_source_library: (args: { providerId: ProviderId }) =>
		runGeneratedCommand(
			generatedCommands.loadRemoteSourceLibrary(args.providerId),
			normalizeNullish,
		),
	list_work_operations: (_args?: undefined) =>
		runGeneratedCommand(generatedCommands.listWorkOperations(), normalizeOperationListSnapshot),
	cancel_work_operation: (args: { operationId: OperationId; childJobId?: string }) =>
		runGeneratedCommand(
			generatedCommands.cancelWorkOperation(args.operationId, args.childJobId ?? null),
			normalizeOperationSnapshot,
		),
	log_frontend: (args: { entry: FrontendLogEntry }) =>
		runGeneratedCommand(generatedCommands.logFrontend(args.entry)),
} as const;

export type TauriCommand = keyof typeof commandSpecs;
export type CommandResult<K extends TauriCommand> = Awaited<ReturnType<(typeof commandSpecs)[K]>>;
