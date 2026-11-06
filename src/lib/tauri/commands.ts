import { commands as generatedCommands } from '../generated/tauri';
import type { SettingsIntent } from '../../types/appSettings';
import type { FrontendLogEntry } from '../../types/frontendLog';
import type { SessionIntent } from '../../types/session';
import type { OperationId } from '../../types/workRuntime';
import { unwrapGeneratedResult } from './appError';

type GeneratedResult<T> = { status: 'ok'; data: T } | { status: 'error'; error: unknown };

async function runGeneratedCommand<T>(promise: Promise<GeneratedResult<T>>): Promise<T> {
	return unwrapGeneratedResult(await promise);
}

export const commandSpecs = {
	attach_frontend: (_args?: undefined) => runGeneratedCommand(generatedCommands.attachFrontend()),
	session_dispatch: (args: { client: number; sequence: number; intent: SessionIntent }) =>
		runGeneratedCommand(generatedCommands.sessionDispatch(args.client, args.sequence, args.intent)),
	open_preview_output: (args: { client: number; sequence: number; runId: string }) =>
		runGeneratedCommand(
			generatedCommands.openPreviewOutput(args.client, args.sequence, args.runId),
		),
	settings_dispatch: (args: { client: number; sequence: number; intent: SettingsIntent }) =>
		runGeneratedCommand(
			generatedCommands.settingsDispatch(args.client, args.sequence, args.intent),
		),
	cancel_work_operation: (args: { operationId: OperationId; childJobId?: string }) =>
		runGeneratedCommand(
			generatedCommands.cancelWorkOperation(args.operationId, args.childJobId ?? null),
		),
	log_frontend: (args: { entry: FrontendLogEntry }) =>
		runGeneratedCommand(generatedCommands.logFrontend(args.entry)),
} as const;

export type TauriCommand = keyof typeof commandSpecs;
export type CommandResult<K extends TauriCommand> = Awaited<ReturnType<(typeof commandSpecs)[K]>>;
