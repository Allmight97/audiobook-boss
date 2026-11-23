import { vi } from 'vitest';
import { fakeEngine } from './fakeEngine';

export function defaultInvoke(command: string, args?: unknown): Promise<unknown> {
	const engineArgs = args as {
		client: number;
		sequence: number;
		intent: never;
		filePaths: string[];
	};
	switch (command) {
		case 'attach_frontend':
			return fakeEngine().attach();
		case 'session_dispatch':
			return fakeEngine().sessionDispatch(
				engineArgs.client,
				engineArgs.sequence,
				engineArgs.intent,
			);
		case 'settings_dispatch':
			return fakeEngine().settingsDispatch(
				engineArgs.client,
				engineArgs.sequence,
				engineArgs.intent,
			);
		default:
			throw new Error(`[Test Mock] Unhandled Tauri invoke: ${command}`);
	}
}

export const invokeMock = vi.fn(defaultInvoke);
