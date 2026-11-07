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

// oxlint-disable-next-line typescript/no-unsafe-type-assertion -- the generated commands differ in arity; each is called only with its own arguments.
const callable = generatedCommands as Record<
	string,
	(...args: unknown[]) => Promise<GeneratedResult<unknown>>
>;

// oxlint-disable-next-line typescript/no-unsafe-type-assertion -- Object.fromEntries loses the per-command types that UnwrappedCommands restores.
export const commands = Object.fromEntries(
	Object.entries(callable).map(([name, command]) => [
		name,
		async (...args: unknown[]) => unwrapGeneratedResult(await command(...args)),
	]),
) as UnwrappedCommands;
