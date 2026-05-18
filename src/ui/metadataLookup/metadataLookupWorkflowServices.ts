import type { tauriClient } from '../../lib/tauri/client';
import {
	type AppLayer,
	makeWorkflowLayer,
	makeWorkflowServiceTag,
} from '../../lib/effect/appEffect';
import type { FileListInfo } from '../../types/audio';
import type { AudiobookMetadata } from '../../types/metadata';
import type { selectFile } from '../fileList/actions';
import type { applyMetadataToForm, readMetadataForm } from '../metadataForm';
import type { setMetadataForFile } from '../metadataState';
import type { updateEstimatedSize, updateOutputPath } from '../outputPanel';
import type { updateTagPreview } from '../tagPreview';
import type {
	MetadataLookupQueueItem,
	MetadataLookupQueueState,
	MetadataLookupState,
} from './state.svelte';

export interface MetadataLookupWorkflowServices {
	getLookupState: () => MetadataLookupState;
	getQueueState: () => MetadataLookupQueueState;
	setMetadataLookupQueue: (queue: MetadataLookupQueueItem[]) => void;
	clearMetadataLookupQueue: () => void;
	setMetadataLookupQueueIndex: (index: number) => void;
	getSelectedFileIndices: () => Set<number>;
	getCurrentFileList: () => FileListInfo | null;
	getMetadataForFile: (filePath: string) => Partial<AudiobookMetadata> | undefined;
	setMetadataForFile: typeof setMetadataForFile;
	selectFile: typeof selectFile;
	applyMetadataToForm: typeof applyMetadataToForm;
	readMetadataForm: typeof readMetadataForm;
	updateOutputPath: typeof updateOutputPath;
	updateEstimatedSize: typeof updateEstimatedSize;
	updateTagPreview: typeof updateTagPreview;
	clearCoverArt: () => void;
	setCoverArt: (coverArtBytes: number[] | null) => void;
	setCustomCoverArt: (coverArtBytes: number[] | null) => void;
	searchOnlineMetadata: typeof tauriClient.searchOnlineMetadata;
	loadCoverArtFromUrl: typeof tauriClient.loadCoverArtFromUrl;
	focusElementById: (id: string) => void;
	queueMicrotask: (callback: () => void) => void;
	console: Pick<Console, 'error' | 'warn'>;
}

export type MetadataLookupWorkflowServicesId = 'MetadataLookup/WorkflowServices';
export type MetadataLookupWorkflowLayer = AppLayer<MetadataLookupWorkflowServicesId>;

export const MetadataLookupWorkflowServicesTag = makeWorkflowServiceTag<
	MetadataLookupWorkflowServicesId,
	MetadataLookupWorkflowServices
>('MetadataLookup/WorkflowServices');

export function makeMetadataLookupWorkflowServicesLayer(
	services: MetadataLookupWorkflowServices,
): MetadataLookupWorkflowLayer {
	return makeWorkflowLayer(MetadataLookupWorkflowServicesTag, services);
}
