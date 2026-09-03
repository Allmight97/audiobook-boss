import type { FileListInfo } from '../../types/audio';
import type { AudiobookMetadata } from '../../types/metadata';
import type { InputOwner } from '../inputSession';
import type { MetadataOwner } from '../metadataSession';
import type { MetadataLookupCoverPreviews } from './coverPreview';
import type { MetadataLookupQueueState, MetadataLookupState } from './state';
import type { MetadataLookupWorkflowServices } from './workflow';

export function makeProductionLookupServices(
	deps: {
		readonly input: InputOwner;
		readonly metadata: MetadataOwner;
		readonly lookupState: MetadataLookupState;
		readonly queueState: MetadataLookupQueueState;
		readonly coverPreviews: MetadataLookupCoverPreviews;
	},
	publishView?: () => void,
): MetadataLookupWorkflowServices {
	return {
		getLookupState: () => deps.lookupState,
		getQueueState: () => deps.queueState,
		setMetadataLookupQueue(queue) {
			deps.queueState.queue = queue;
			deps.queueState.index = 0;
		},
		clearMetadataLookupQueue() {
			deps.queueState.queue = [];
			deps.queueState.index = 0;
		},
		setMetadataLookupQueueIndex(index) {
			deps.queueState.index = index;
		},
		getSelectedFileIndices: () => new Set(deps.input.session().selectedIndices ?? []),
		getCurrentFileList: (): FileListInfo | null => deps.input.session().fileList ?? null,
		getMetadataForFile: (path) => deps.metadata.readCached(path),
		stageMetadataIntentPatch: (path, patch) => deps.metadata.stageIntent(path, patch),
		selectFile: async (index, modifiers, options) => {
			const changed = await deps.input.selectFile({
				index,
				modifiers: modifiers ?? { multi: false, range: false },
				skipPersistPrevious: options?.skipPersistPrevious,
			});
			if (changed === false) {
				return;
			}
			await deps.metadata.hydrateSelection(document.activeElement);
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
		loadLookupCoverBytes: (url) => deps.coverPreviews.loadBytes(url),
		clearCoverPreviews: () => deps.coverPreviews.clear(),
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
