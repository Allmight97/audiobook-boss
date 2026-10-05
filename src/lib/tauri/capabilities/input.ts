import type { UnlistenFn } from '@tauri-apps/api/event';
import type { SupportedAudioImportMetadata } from '../../../types/audio';
import { tauriClient } from '../client';

export interface InputOpenFileOptions {
	readonly filters?: ReadonlyArray<{
		readonly name: string;
		readonly extensions: ReadonlyArray<string>;
	}>;
}

export interface NativeDropPayload {
	readonly paths: ReadonlyArray<string>;
	readonly position: { readonly x: number; readonly y: number };
}

export type InputUnlisten = UnlistenFn;

export interface InputCapability {
	openFiles(options?: InputOpenFileOptions): Promise<string[] | null>;
	openDirectory(): Promise<string | null>;
	getSupportedAudioImportMetadata(): Promise<SupportedAudioImportMetadata>;
	listenDragDrop(handler: (payload: NativeDropPayload) => void): Promise<InputUnlisten>;
	listenDragEnter(handler: () => void): Promise<InputUnlisten>;
	listenDragLeave(handler: () => void): Promise<InputUnlisten>;
}

export const liveInputCapability: InputCapability = {
	openFiles: (options) =>
		tauriClient.openFiles(
			options
				? {
						filters: options.filters?.map((filter) => ({
							name: filter.name,
							extensions: [...filter.extensions],
						})),
					}
				: undefined,
		),
	openDirectory: () => tauriClient.openDirectory(),
	getSupportedAudioImportMetadata: () => tauriClient.getSupportedAudioImportMetadata(),
	listenDragDrop: (handler) =>
		tauriClient.listen('tauri://drag-drop', (event) => {
			handler(event.payload);
		}),
	listenDragEnter: (handler) =>
		tauriClient.listen('tauri://drag-enter', () => {
			handler();
		}),
	listenDragLeave: (handler) =>
		tauriClient.listen('tauri://drag-leave', () => {
			handler();
		}),
};
