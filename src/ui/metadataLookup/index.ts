// Metadata Lookup Public API Strip. Enumerated in AGENTS.md (no contract test
// pins this owner yet — keep the doc list in sync on purpose-changes).
export {
	applyMetadataLookupResult,
	closeMetadataLookup,
	initMetadataLookup,
	openMetadataLookup,
	searchMetadataLookup,
	skipMetadataLookupQueueItem,
	useManualMetadataEntryFromLookup,
} from './actions';
export { default as MetadataLookupIsland } from './MetadataLookupIsland.svelte';
