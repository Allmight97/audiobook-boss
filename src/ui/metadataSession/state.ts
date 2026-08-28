export {
	cacheMetadataForFile,
	clearMetadataSession,
	clearPendingMetadataForFile,
	collectActionableMetadataIntent,
	getMetadataForFile,
	getMetadataIntentPatchForFile,
	getPendingMetadataIntentEntries,
	isUsableMetadataCache,
	removeMetadataForFile,
	stageMetadataIntentPatch,
} from '../../app/metadataSession/cache';
export type { MetadataStageResult } from '../../app/metadataSession/cache';
