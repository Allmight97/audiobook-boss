import type { FileListInfo } from '../../types/audio';
import type { AudiobookMetadata } from '../../types/metadata';
import { coverArtBytesToDataUrl } from '../../lib/media/coverArtDataUrl';
import type { MetadataCapability } from '../../lib/tauri/capabilities/metadata';
import { inputSessionAtom } from '../inputSession/atoms';
import { selectFileInSession } from '../inputSession/selection';
import type { InputSessionState } from '../inputSession/types';
import {
	clearCoverArtAtom,
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
	(atom: typeof inputSessionAtom): InputSessionState;
	(atom: typeof metadataEditorAtom): MetadataEditorState;
	(atom: typeof metadataCapabilityAtom): MetadataCapability;
	readonly set: {
		(atom: typeof inputSessionAtom, value: InputSessionState): void;
		(atom: typeof metadataEditorAtom, value: MetadataEditorState): void;
		(atom: typeof clearCoverArtAtom, value: undefined): void;
		(atom: typeof setCustomCoverArtAtom, value: number[]): void;
	};
};

export function makeProductionLookupServices(
	get: LookupServiceGet,
): MetadataLookupWorkflowServices {
	return {
		getLookupState: () => metadataLookupState,
		getQueueState: () => metadataLookupQueueState,
		setMetadataLookupQueue,
		clearMetadataLookupQueue,
		setMetadataLookupQueueIndex,
		getSelectedFileIndices: () => new Set(get(inputSessionAtom).selectedIndices),
		getCurrentFileList: (): FileListInfo | null => get(inputSessionAtom).fileList,
		getMetadataForFile,
		stageMetadataIntentPatch,
		selectFile: (index, modifiers) => {
			get.set(
				inputSessionAtom,
				selectFileInSession(
					get(inputSessionAtom),
					index,
					modifiers ?? { multi: false, range: false },
				),
			);
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
		updateOutputPath: () => undefined,
		updateEstimatedSize: () => undefined,
		clearCoverArt: () => {
			get.set(clearCoverArtAtom, undefined);
		},
		setCoverArt: () => undefined,
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
		refreshCoverArtDisplay: () => undefined,
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
	};
}
