import type { FileListInfo } from '../../types/audio';
import type { AudiobookMetadata } from '../../types/metadata';
import { coverArtBytesToDataUrl } from '../../lib/media/coverArtDataUrl';
import type { MetadataCapability } from '../../lib/tauri/capabilities/metadata';
import type { InputOwner } from '../inputSession/owner';
import {
	getMetadataForFile,
	metadataCapabilityAtom,
	metadataEditorAtom,
	setCustomCoverArtAtom,
	stageMetadataIntentPatch,
	type MetadataEditorState,
} from '../metadataSession';
import { applyMetadataToForm, readMetadataForm } from '../metadataSession/form';
import {
	clearMetadataLookupQueue,
	metadataLookupQueueState,
	metadataLookupState,
	setMetadataLookupQueue,
	setMetadataLookupQueueIndex,
} from './state';
import type { MetadataLookupWorkflowServices } from './workflow';

export type LookupServiceGet = {
	(atom: typeof metadataEditorAtom): MetadataEditorState;
	(atom: typeof metadataCapabilityAtom): MetadataCapability;
	readonly set: {
		(atom: typeof metadataEditorAtom, value: MetadataEditorState): void;
		(atom: typeof setCustomCoverArtAtom, value: number[]): void;
	};
};

let boundInput: InputOwner | undefined;

export function bindLookupInput(input: InputOwner | undefined): void {
	boundInput = input;
}

export function makeProductionLookupServices(
	get: LookupServiceGet,
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
		selectFile: (index, modifiers) => {
			boundInput?.selectFile({
				index,
				modifiers: modifiers ?? { multi: false, range: false },
			});
		},
		applyMetadataToForm: (metadata: Partial<AudiobookMetadata>, options) => {
			const editor = get(metadataEditorAtom);
			get.set(metadataEditorAtom, {
				...editor,
				form: applyMetadataToForm(editor.form, metadata, {
					mode: options?.mode ?? 'single',
					markDirty: options?.markDirty ?? true,
				}),
				formRevision: editor.formRevision + 1,
			});
		},
		readMetadataForm: (options) => {
			const editor = get(metadataEditorAtom);
			return readMetadataForm(editor.form, {
				mode: options?.mode,
				includeCoverArt: options?.includeCoverArt,
				coverArtBytes: editor.cover.currentCoverArt,
				coverArtRemovalRequested: editor.cover.coverArtRemovalRequested,
			});
		},
		setCustomCoverArt: (coverArtBytes) => {
			if (!coverArtBytes || coverArtBytes.length === 0) return;
			get.set(setCustomCoverArtAtom, coverArtBytes);
			const editor = get(metadataEditorAtom);
			get.set(metadataEditorAtom, {
				...editor,
				cover: {
					...editor.cover,
					currentCoverArt: coverArtBytes,
					imageDataUrl: coverArtBytesToDataUrl(coverArtBytes),
					hasCustomCoverArt: true,
					coverArtRemovalRequested: false,
				},
				coverRevision: editor.coverRevision + 1,
			});
		},
		searchOnlineMetadata: (args) => get(metadataCapabilityAtom).searchOnlineMetadata(args),
		loadCoverArtFromUrl: (url) => get(metadataCapabilityAtom).loadCoverArtFromUrl(url),
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
