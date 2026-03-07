import type { MetadataSource, OnlineMetadataResult } from '../../types/metadata';
import type { AudioFile } from '../../types/audio';

export type MetadataLookupStatusVariant = 'error' | 'success' | 'info';
export type MetadataLookupApplyMode = 'current' | 'queue';
export type MetadataLookupSource = 'auto' | MetadataSource;
type MetadataLookupQueueItem = {
	file: AudioFile;
	index: number;
};

type MetadataLookupQueueState = {
	queue: MetadataLookupQueueItem[];
	index: number;
};

function createMetadataLookupQueueState(): MetadataLookupQueueState {
	return {
		queue: [],
		index: 0,
	};
}

export const metadataLookupState = $state({
	isOpen: false,
	query: '',
	source: 'auto' as MetadataLookupSource,
	applyMode: 'current' as MetadataLookupApplyMode,
	replaceCoverArt: false,
	statusMessage: '',
	statusVariant: 'info' as MetadataLookupStatusVariant,
	queueContext: 'No files selected.',
	results: [] as OnlineMetadataResult[],
	isQueueMode: false,
	skipEnabled: false,
	hasSearched: false,
});

export const metadataLookupQueueState = $state<MetadataLookupQueueState>(
	createMetadataLookupQueueState(),
);

export function setMetadataLookupQueue(queue: MetadataLookupQueueItem[]): void {
	metadataLookupQueueState.queue = queue;
	metadataLookupQueueState.index = 0;
}

export function clearMetadataLookupQueue(): void {
	metadataLookupQueueState.queue = [];
	metadataLookupQueueState.index = 0;
}

export function setMetadataLookupQueueIndex(index: number): void {
	metadataLookupQueueState.index = index;
}
