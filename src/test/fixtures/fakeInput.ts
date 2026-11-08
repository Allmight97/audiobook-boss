import { vi } from 'vitest';
import type { InputCapability, NativeDropPayload } from '../../lib/tauri/capabilities/input';

export type FakeInput = InputCapability & {
	drop(paths: string[], position?: NativeDropPayload['position']): void;
	readonly dropListening: () => boolean;
};

export function fakeInput(overrides: Partial<InputCapability> = {}): FakeInput {
	let dropHandler: ((payload: NativeDropPayload) => void) | undefined;
	return {
		openFiles: vi.fn(async () => []),
		openDirectory: vi.fn(async () => null),
		listenDragDrop: vi.fn(async (handler) => {
			dropHandler = handler;
			return () => {
				dropHandler = undefined;
			};
		}),
		listenDragEnter: vi.fn(async () => () => undefined),
		listenDragLeave: vi.fn(async () => () => undefined),
		...overrides,
		drop(paths, position = { x: 0, y: 0 }) {
			if (!dropHandler) throw new Error('nothing is listening for drops');
			dropHandler({ paths, position });
		},
		dropListening: () => dropHandler !== undefined,
	};
}
