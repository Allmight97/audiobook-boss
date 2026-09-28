import type {
	AudiobookMetadata,
	MetadataLookupResponse,
	MetadataSaveBatchResult,
	MetadataSaveRequest,
	MetadataSource,
} from '../../../types/metadata';
import type {
	MetadataIntentPatch,
	MetadataIntentValidationResult,
} from '../../../types/metadataIntent';
import { tauriClient } from '../client';

export interface MetadataOpenFileOptions {
	readonly title?: string;
	readonly filters?: ReadonlyArray<{
		readonly name: string;
		readonly extensions: ReadonlyArray<string>;
	}>;
}

export interface MetadataCapability {
	readAudioMetadata(filePath: string): Promise<Partial<AudiobookMetadata>>;
	validateMetadataIntentPatch(patch: MetadataIntentPatch): Promise<MetadataIntentValidationResult>;
	/** The album sort (TSOA) processing would write for `metadata`. */
	previewAlbumSort(metadata: Partial<AudiobookMetadata>): Promise<string | null>;
	saveMetadataBatch(items: ReadonlyArray<MetadataSaveRequest>): Promise<MetadataSaveBatchResult>;
	openFile(options?: MetadataOpenFileOptions): Promise<string | null>;
	loadCoverArtFile(filePath: string): Promise<number[]>;
	loadCoverArtFromUrl(url: string): Promise<number[]>;
	searchOnlineMetadata(args: {
		query: string;
		sources: MetadataSource[] | null;
		limit?: number | null;
	}): Promise<MetadataLookupResponse>;
}

export const liveMetadataCapability: MetadataCapability = {
	readAudioMetadata: (filePath) => tauriClient.readAudioMetadata(filePath),
	validateMetadataIntentPatch: (patch) => tauriClient.validateMetadataIntentPatch(patch),
	previewAlbumSort: (metadata) => tauriClient.previewAlbumSort(metadata),
	saveMetadataBatch: (items) => tauriClient.saveMetadataBatch([...items]),
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
	loadCoverArtFile: (filePath) => tauriClient.loadCoverArtFile(filePath),
	loadCoverArtFromUrl: (url) => tauriClient.loadCoverArtFromUrl(url),
	searchOnlineMetadata: (args) => tauriClient.searchOnlineMetadata(args),
};
