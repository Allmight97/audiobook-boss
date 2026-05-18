import type { MetadataSource, OnlineMetadataResult } from '../../types/metadata';
import type { AudioFile } from '../../types/audio';

export type MetadataLookupStatusVariant = 'error' | 'success' | 'info';
export type MetadataLookupApplyMode = 'current' | 'queue';
export type MetadataLookupSource = 'auto' | MetadataSource;
export type MetadataLookupQueueItem = {
	file: AudioFile;
	index: number;
};

export type MetadataLookupQueueState = {
	queue: MetadataLookupQueueItem[];
	index: number;
};

export type MetadataLookupState = {
	isOpen: boolean;
	query: string;
	source: MetadataLookupSource;
	applyMode: MetadataLookupApplyMode;
	replaceCoverArt: boolean;
	statusMessage: string;
	statusVariant: MetadataLookupStatusVariant;
	queueContext: string;
	results: OnlineMetadataResult[];
	isQueueMode: boolean;
	skipEnabled: boolean;
	hasSearched: boolean;
};

function createMetadataLookupQueueState(): MetadataLookupQueueState {
	return {
		queue: [],
		index: 0,
	};
}

export const metadataLookupState = $state<MetadataLookupState>({
	isOpen: false,
	query: '',
	source: 'auto',
	applyMode: 'current',
	replaceCoverArt: false,
	statusMessage: '',
	statusVariant: 'info',
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
