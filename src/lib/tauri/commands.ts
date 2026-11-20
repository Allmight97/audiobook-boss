import { commands as generatedCommands } from '../generated/tauri';
import { unwrapGeneratedResult } from './appError';

type GeneratedResult<T> = { status: 'ok'; data: T } | { status: 'error'; error: unknown };
type GeneratedCommands = typeof generatedCommands;
type Unwrapped<Reply> = Reply extends Promise<GeneratedResult<infer Data>> ? Promise<Data> : never;

export type UnwrappedCommands = {
	[Name in keyof GeneratedCommands]: (
		...args: Parameters<GeneratedCommands[Name]>
	) => Unwrapped<ReturnType<GeneratedCommands[Name]>>;
};

function unwrap<Args extends readonly unknown[], Data>(
	command: (...args: Args) => Promise<GeneratedResult<Data>>,
): (...args: Args) => Promise<Data> {
	return async (...args) => unwrapGeneratedResult(await command(...args));
}

export const commands = {
	attachFrontend: unwrap(generatedCommands.attachFrontend),
	sessionDispatch: unwrap(generatedCommands.sessionDispatch),
	openPreviewOutput: unwrap(generatedCommands.openPreviewOutput),
	settingsDispatch: unwrap(generatedCommands.settingsDispatch),
	cancelWorkOperation: unwrap(generatedCommands.cancelWorkOperation),
	logFrontend: unwrap(generatedCommands.logFrontend),
} satisfies UnwrappedCommands;
