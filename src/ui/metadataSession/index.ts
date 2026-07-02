// Metadata Session Public API Strip. Pinned by
// __tests__/runtime-api-contract.test.ts — change that test when this surface
// changes on purpose.

// Cache truth
export { cacheMetadataForFile, getMetadataForFile, isUsableMetadataCache } from './state';
// Pending intent truth
export {
	clearMetadataSession,
	collectActionableMetadataIntent,
	getMetadataIntentPatchForFile,
	removeMetadataForFile,
	stageMetadataIntentPatch,
} from './state';
export type { MetadataStageResult } from './state';
// Draft / validation
export { buildMetadataDraftIntent } from './draft';
export type { MetadataDraft, MetadataDraftField } from './draft';
export { validateMetadataDraft } from './validation';
export type { MetadataDraftValidation, ValidateMetadataIntentPatch } from './validation';
// Save
export { saveMetadataFromUI } from './saveWorkflow';
export { metadataSaveInProgress } from './saveState';
