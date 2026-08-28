import type { FileListInfo, SupportedAudioImportMetadata } from '../../../types/audio';
import { tauriClient } from '../client';

export interface InputOpenFileOptions {
	readonly filters?: ReadonlyArray<{
		readonly name: string;
		readonly extensions: ReadonlyArray<string>;
	}>;
}

export interface InputCapability {
	openFiles(options?: InputOpenFileOptions): Promise<string[] | null>;
	openDirectory(): Promise<string | null>;
	discoverAudioImportPaths(paths: ReadonlyArray<string>): Promise<string[]>;
	analyzeAudioFiles(paths: ReadonlyArray<string>): Promise<FileListInfo>;
	getSupportedAudioImportMetadata(): Promise<SupportedAudioImportMetadata>;
	takeOpenedAudioFiles(): Promise<string[]>;
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
};
