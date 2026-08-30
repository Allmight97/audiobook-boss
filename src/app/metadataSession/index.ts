export {
	cacheMetadataForFile,
	clearMetadataSession,
	collectActionableMetadataIntent,
	getMetadataForFile,
	getMetadataIntentPatchForFile,
	isUsableMetadataCache,
	removeMetadataForFile,
	stageMetadataIntentPatch,
} from './cache';
export {
	commitPreparedMetadataDrafts,
	prepareMetadataDrafts,
	readUncachedMetadataSnapshot,
} from './staging';
export type { MetadataStageResult } from './cache';
export { buildMetadataDraftIntent } from './draft';
export type { MetadataDraft, MetadataDraftField } from './draft';
export { validateMetadataDraft } from './validation';
export type { MetadataDraftValidation, ValidateMetadataIntentPatch } from './validation';
export {
	bindMetadataInput,
	applyCoverArtDropAtom,
	applyLookupMetadataAtom,
	clearCoverArtAtom,
	hydrateMetadataSelectionAtom,
	loadCoverArtFromPickerAtom,
	loadCoverArtFromUrlAtom,
	metadataCapabilityAtom,
	metadataEditorAtom,
	metadataViewAtom,
	readEditorHasDirtyMetadata,
	readEditorMetadata,
	saveMetadataAtom,
	setCoverDragOverAtom,
	setCoverHoveredAtom,
	setCoverUrlInputAtom,
	setCustomCoverArtAtom,
	setMetadataFieldActionAtom,
	setMetadataFieldValueAtom,
} from './atoms';
export type { MetadataEditorState, MetadataView } from './atoms';
export { hasDirtyMetadataFields, readMetadataForm } from './form';
export { COVER_ART_IMAGE_EXTENSION_HINTS } from './cover';
export type { CoverArtMessage } from './cover';
export { calculateTSOA, projectTagPreviewValues } from './tags';
export type { TagField, TagPreviewValues } from './tags';
export { METADATA_FIELD_DEFINITIONS } from './fields';
export type { MetadataFieldId, MetadataFormMode, MetadataFormState } from './fields';
