import { tauriClient } from '../client';

export interface MetadataOpenFileOptions {
	readonly title?: string;
	readonly filters?: ReadonlyArray<{
		readonly name: string;
		readonly extensions: ReadonlyArray<string>;
	}>;
}

/** Host services the metadata views use directly: the image picker, and cover previews. */
export interface MetadataCapability {
	openFile(options?: MetadataOpenFileOptions): Promise<string | null>;
	loadCoverArtFromUrl(url: string): Promise<number[]>;
}

export const liveMetadataCapability: MetadataCapability = {
	openFile: (options) =>
		tauriClient.openFile(
			options
				? {
						title: options.title,
						filters: options.filters?.map((filter) => ({
							name: filter.name,
							extensions: [...filter.extensions],
						})),
					}
				: undefined,
		),
	loadCoverArtFromUrl: (url) => tauriClient.loadCoverArtFromUrl(url),
};
