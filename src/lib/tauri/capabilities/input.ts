import type { UnlistenFn } from '@tauri-apps/api/event';
import type { FileListInfo, SupportedAudioImportMetadata } from '../../../types/audio';
import { EVENTS } from '../../../types/events';
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
	discoverAudioImportPaths(paths: ReadonlyArray<string>): Promise<string[]>;
	analyzeAudioFiles(paths: ReadonlyArray<string>): Promise<FileListInfo>;
	getSupportedAudioImportMetadata(): Promise<SupportedAudioImportMetadata>;
	takeOpenedAudioFiles(): Promise<string[]>;
	readAudioCoverThumbnail(path: string): Promise<ReadonlyArray<number> | null | undefined>;
	listenDragDrop(handler: (payload: NativeDropPayload) => void): Promise<InputUnlisten>;
	listenDragEnter(handler: () => void): Promise<InputUnlisten>;
	listenDragLeave(handler: () => void): Promise<InputUnlisten>;
	listenOpenedAudioFiles(handler: () => void): Promise<InputUnlisten>;
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
	discoverAudioImportPaths: (paths) => tauriClient.discoverAudioImportPaths([...paths]),
	analyzeAudioFiles: (paths) => tauriClient.analyzeAudioFiles([...paths]),
	getSupportedAudioImportMetadata: () => tauriClient.getSupportedAudioImportMetadata(),
	takeOpenedAudioFiles: () => tauriClient.takeOpenedAudioFiles(),
	readAudioCoverThumbnail: (path) => tauriClient.readAudioCoverThumbnail(path),
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
	listenOpenedAudioFiles: (handler) =>
		tauriClient.listen(EVENTS.OPENED_AUDIO_FILES, () => {
			handler();
		}),
};
