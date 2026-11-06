import {
	commands as generatedCommands,
	type EncoderDefaults as GeneratedEncoderDefaults,
	type OutputDefaults as GeneratedOutputDefaults,
	type SettingsIntent as GeneratedSettingsIntent,
	type OutputNamingConfig as GeneratedOutputNamingConfig,
} from '../generated/tauri';
import type { OutputNamingConfig } from '../../types/audio';
import type { AppSettings, SettingsIntent } from '../../types/appSettings';
import type { FrontendLogEntry } from '../../types/frontendLog';
import type { SessionIntent } from '../../types/session';
import type { OperationId } from '../../types/workRuntime';
import { normalizeAppError, unwrapGeneratedResult } from './appError';
import {
	denormalizeNullish,
	normalizeFrontendAttachment,
	normalizeWorkOperationsSnapshot,
	normalizeOperationSnapshot,
	normalizeSessionReply,
	normalizeSettingsReply,
} from './normalizers';

type GeneratedResult<T> = { status: 'ok'; data: T } | { status: 'error'; error: unknown };

async function runGeneratedCommand<T>(promise: Promise<GeneratedResult<T>>): Promise<T>;
async function runGeneratedCommand<T, R>(
	promise: Promise<GeneratedResult<T>>,
	transform: (value: T) => R,
): Promise<R>;
async function runGeneratedCommand<T, R>(
	promise: Promise<GeneratedResult<T>>,
	transform?: (value: T) => R,
): Promise<T | R> {
	try {
		const unwrapped = unwrapGeneratedResult(await promise);
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
	open_preview_output: (args: { client: number; sequence: number; runId: string }) =>
		runGeneratedCommand(
			generatedCommands.openPreviewOutput(args.client, args.sequence, args.runId),
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
	list_work_operations: (_args?: undefined) =>
		runGeneratedCommand(generatedCommands.listWorkOperations(), normalizeWorkOperationsSnapshot),
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
