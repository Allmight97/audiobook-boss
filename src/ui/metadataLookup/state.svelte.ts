import type { MetadataSource, OnlineMetadataResult } from '../../types/metadata';

export type MetadataLookupStatusVariant = 'error' | 'success' | 'info';
export type MetadataLookupApplyMode = 'current' | 'queue';

export const metadataLookupState = $state({
	isOpen: false,
	query: '',
	source: 'audnexus' as MetadataSource,
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
