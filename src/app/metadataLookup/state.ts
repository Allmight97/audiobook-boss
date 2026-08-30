import type { AudioFile } from '../../types/audio';
import type { MetadataSource, OnlineMetadataResult } from '../../types/metadata';

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
	/** Search criteria stay separated so users can see and fix each part;
	 * they are joined only when the search request is built. */
	titleQuery: string;
	authorQuery: string;
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

export function createMetadataLookupState(): MetadataLookupState {
	return {
		isOpen: false,
		titleQuery: '',
		authorQuery: '',
		source: 'auto',
		applyMode: 'current',
		replaceCoverArt: false,
		statusMessage: '',
		statusVariant: 'info',
		queueContext: 'No files selected.',
		results: [],
		isQueueMode: false,
		skipEnabled: false,
		hasSearched: false,
	};
}

export function createMetadataLookupQueueState(): MetadataLookupQueueState {
	return {
		queue: [],
		index: 0,
	};
}

export function snapshotMetadataLookupState(state: MetadataLookupState): MetadataLookupState {
	return {
		...state,
		results: [...state.results],
	};
}
