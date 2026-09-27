import type { InputOwner } from '../inputSession';
import type { MetadataOwner } from '../metadataSession';
import type { CoverArtPreviewScheduler } from '../../lib/media/coverArtPreviewScheduler';
import type { MetadataLookupQueueState, MetadataLookupState } from './state';
import type { MetadataLookupWorkflowServices } from './workflow';

export function makeProductionLookupServices(
	deps: {
		readonly input: InputOwner;
		readonly metadata: MetadataOwner;
		readonly lookupState: MetadataLookupState;
		readonly queueState: MetadataLookupQueueState;
		readonly coverPreviews: CoverArtPreviewScheduler;
		readonly signal: AbortSignal;
	},
	publishView?: () => void,
): MetadataLookupWorkflowServices {
	return {
		isCurrent: () => !deps.signal.aborted,
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
		currentTitles: () => deps.input.session().files,
		getMetadataForFile: (path) => deps.metadata.readCached(path),
		selectFile: async (file) => {
			const index = deps.input
				.session()
				.files.findIndex(
					(candidate) => candidate.path === file.path && candidate.inputId === file.inputId,
				);
			if (
				index < 0 ||
				!(await deps.input.selectFile({
					index,
					modifiers: { multi: false, range: false },
					signal: deps.signal,
				})) ||
				deps.signal.aborted
			)
				return false;
			return deps.metadata.hydrateSelection(document.activeElement);
		},
		applyMetadataToForm: (file, metadata, coverArtBytes) =>
			deps.metadata.applyLookupMetadata(file, metadata, coverArtBytes),
		searchOnlineMetadata: (args) => deps.metadata.capability().searchOnlineMetadata(args),
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
