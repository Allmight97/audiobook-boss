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

export function makeProductionLookupServices(
	deps: {
		readonly input: InputOwner;
		readonly metadata: MetadataOwner;
	},
	publishView?: () => void,
): MetadataLookupWorkflowServices {
	return {
		getLookupState: () => metadataLookupState,
		getQueueState: () => metadataLookupQueueState,
		setMetadataLookupQueue,
		clearMetadataLookupQueue,
		setMetadataLookupQueueIndex,
		getSelectedFileIndices: () => new Set(deps.input.session().selectedIndices ?? []),
		getCurrentFileList: (): FileListInfo | null => deps.input.session().fileList ?? null,
		getMetadataForFile,
		stageMetadataIntentPatch,
		selectFile: async (index, modifiers, options) => {
			await deps.input.selectFile({
				index,
				modifiers: modifiers ?? { multi: false, range: false },
				skipPersistPrevious: options?.skipPersistPrevious,
			});
		},
		applyMetadataToForm: (metadata: Partial<AudiobookMetadata>) => {
			deps.metadata.applyLookupMetadata(metadata);
		},
		readMetadataForm: () => deps.metadata.readMetadata() ?? {},
		setCustomCoverArt: (coverArtBytes) => {
			deps.metadata.setCustomCoverArt(coverArtBytes);
		},
		searchOnlineMetadata: (args) => deps.metadata.capability().searchOnlineMetadata(args),
		loadCoverArtFromUrl: (url) => deps.metadata.capability().loadCoverArtFromUrl(url),
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
