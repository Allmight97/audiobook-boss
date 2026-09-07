import type { FileListInfo } from '../../types/audio';
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
		getCurrentFileList: (): FileListInfo | null => deps.input.session().fileList ?? null,
		getMetadataForFile: (path) => deps.metadata.readCached(path),
		stageMetadataIntentPatch: (path, patch) => deps.metadata.stageIntent(path, patch),
		selectFile: async (file) => {
			const index =
				deps.input
					.session()
					.fileList?.files.findIndex(
						(candidate) => candidate.path === file.path && candidate.inputId === file.inputId,
					) ?? -1;
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
		readMetadataForm: () => deps.metadata.readMetadata() ?? {},
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
