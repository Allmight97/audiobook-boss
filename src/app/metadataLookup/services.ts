import type { FileListInfo } from '../../types/audio';
import type { AudiobookMetadata } from '../../types/metadata';
import type { InputOwner } from '../inputSession/owner';
import type { MetadataOwner } from '../metadataSession/owner';
import { getMetadataForFile, stageMetadataIntentPatch } from '../metadataSession';
import {
	clearMetadataLookupQueue,
	metadataLookupQueueState,
	metadataLookupState,
	setMetadataLookupQueue,
	setMetadataLookupQueueIndex,
} from './state';
import type { MetadataLookupWorkflowServices } from './workflow';

export type LookupServiceGet = Record<string, never>;

let boundInput: InputOwner | undefined;
let boundMetadata: MetadataOwner | undefined;

export function bindLookupInput(input: InputOwner | undefined): void {
	boundInput = input;
}

export function bindLookupMetadata(metadata: MetadataOwner | undefined): void {
	boundMetadata = metadata;
}

export function makeProductionLookupServices(
	_get: LookupServiceGet,
	publishView?: () => void,
): MetadataLookupWorkflowServices {
	return {
		getLookupState: () => metadataLookupState,
		getQueueState: () => metadataLookupQueueState,
		setMetadataLookupQueue,
		clearMetadataLookupQueue,
		setMetadataLookupQueueIndex,
		getSelectedFileIndices: () => new Set(boundInput?.session().selectedIndices ?? []),
		getCurrentFileList: (): FileListInfo | null => boundInput?.session().fileList ?? null,
		getMetadataForFile,
		stageMetadataIntentPatch,
		selectFile: async (index, modifiers, options) => {
			await boundInput?.selectFile({
				index,
				modifiers: modifiers ?? { multi: false, range: false },
				skipPersistPrevious: options?.skipPersistPrevious,
			});
		},
		applyMetadataToForm: (metadata: Partial<AudiobookMetadata>) => {
			boundMetadata?.applyLookupMetadata(metadata);
		},
		readMetadataForm: () => boundMetadata?.readMetadata() ?? {},
		setCustomCoverArt: (coverArtBytes) => {
			boundMetadata?.setCustomCoverArt(coverArtBytes);
		},
		searchOnlineMetadata: (args) => {
			if (!boundMetadata) {
				return Promise.reject(new Error('Metadata owner is not mounted'));
			}
			return boundMetadata.capability().searchOnlineMetadata(args);
		},
		loadCoverArtFromUrl: (url) => {
			if (!boundMetadata) {
				return Promise.reject(new Error('Metadata owner is not mounted'));
			}
			return boundMetadata.capability().loadCoverArtFromUrl(url);
		},
		focusElementById: (id) => {
			const element = document.getElementById(id);
			if (element instanceof HTMLElement) {
				element.focus();
			}
		},
		queueMicrotask,
		console,
		publishView,
	};
}
