import { tauriClient } from '../../lib/tauri/client';
import type { AudioFile } from '../../types/audio';
import { EVENTS, isFileDropEvent } from '../../types/events';
import { applyCoverArtDrop } from '../coverArt';
import { getCurrentFileList, onOrderLockChange } from '../fileList/state.svelte';
import {
	setFileImportDragOver,
	setFileImportError,
	setFileImportSupportText,
} from './state.svelte';
import {
	enterImportAnalysisWorkflow,
	importOrderLockedMessage,
	runImportAnalysisWorkflow,
} from './importAnalysisWorkflow';
import {
	ImportAnalysisWorkflowLive,
	liveImportAnalysisWorkflowServices,
} from './importAnalysisWorkflowLive';

export interface DragDropContext {
	getCoverArtArea: () => HTMLElement | null;
	getFileManagementContainer: () => HTMLElement | null;
	getVisibleFiles: () => AudioFile[];
}

type Unlisten = () => void;

export function attachTauriDragHandlers(context: DragDropContext): Unlisten {
	const { getCoverArtArea, getFileManagementContainer, getVisibleFiles } = context;
	const unlisteners: Unlisten[] = [];
	let isDisposed = false;
	let hasDeferredOpenedAudioDrain = false;

	const registerUnlistener = (unlisten: Unlisten): void => {
		if (isDisposed) {
			unlisten();
			return;
		}
		unlisteners.push(unlisten);
	};

	const captureUnlistener = (result: unknown): void => {
		if (typeof result === 'function') {
			registerUnlistener(result as Unlisten);
			return;
		}
		if (result && typeof (result as PromiseLike<unknown>).then === 'function') {
			void (result as Promise<unknown>).then((unlisten) => {
				if (typeof unlisten === 'function') {
					registerUnlistener(unlisten as Unlisten);
				}
			});
		}
	};

	const drainOpenedAudioFiles = async (): Promise<void> => {
		const drained = await handleOpenedAudioFiles(getVisibleFiles());
		hasDeferredOpenedAudioDrain = !drained;
	};

	const dragDropHandler = async (event: {
		payload: { position: { x: number; y: number }; paths: string[] };
	}) => {
		setFileImportDragOver(false);
		if (!isFileDropEvent(event.payload)) {
			return;
		}

		const coverArea = getCoverArtArea();
		if (coverArea) {
			const rect = coverArea.getBoundingClientRect();
			const { x, y } = event.payload.position;
			if (x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom) {
				const handled = await applyCoverArtDrop(event.payload.paths);
				if (handled) {
					return;
				}
			}
		}

		const fileManagementContainer = getFileManagementContainer();
		if (!fileManagementContainer) {
			return;
		}

		const rect = fileManagementContainer.getBoundingClientRect();
		const { x, y } = event.payload.position;
		if (x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom) {
			await handleFileDrop(event.payload.paths, getVisibleFiles());
		}
	};

	captureUnlistener(tauriClient.listen('tauri://drag-drop', dragDropHandler));
	captureUnlistener(
		tauriClient.listen('tauri://drag-enter', () => {
			setFileImportDragOver(true);
		}),
	);
	captureUnlistener(
		tauriClient.listen('tauri://drag-leave', () => {
			setFileImportDragOver(false);
		}),
	);
	captureUnlistener(tauriClient.listen(EVENTS.OPENED_AUDIO_FILES, drainOpenedAudioFiles));
	registerUnlistener(
		onOrderLockChange((locked) => {
			if (!locked && hasDeferredOpenedAudioDrain) {
				void drainOpenedAudioFiles();
			}
		}),
	);

	void refreshSupportedAudioImportMetadata();
	void drainOpenedAudioFiles();

	return () => {
		isDisposed = true;
		for (const unlisten of unlisteners.splice(0, unlisteners.length)) {
			unlisten();
		}
	};
}

export async function handleClickToSelect(existingFiles: AudioFile[] = []): Promise<void> {
	const currentFiles =
		existingFiles.length > 0 ? existingFiles : (getCurrentFileList()?.files ?? []);
	const action = { type: 'clickToSelect' as const, existingFiles: currentFiles };
	const preparedEntry = enterImportAnalysisWorkflow(liveImportAnalysisWorkflowServices, action);
	if (!preparedEntry) {
		return;
	}
	await runImportAnalysisWorkflow(action, ImportAnalysisWorkflowLive, preparedEntry);
}

export async function handleClickToSelectFolder(existingFiles: AudioFile[] = []): Promise<void> {
	const currentFiles =
		existingFiles.length > 0 ? existingFiles : (getCurrentFileList()?.files ?? []);
	const action = { type: 'clickToSelectFolder' as const, existingFiles: currentFiles };
	const preparedEntry = enterImportAnalysisWorkflow(liveImportAnalysisWorkflowServices, action);
	if (!preparedEntry) {
		return;
	}
	await runImportAnalysisWorkflow(action, ImportAnalysisWorkflowLive, preparedEntry);
}

export async function handleImportedAudioPaths(
	paths: string[],
	existingFiles: AudioFile[] = [],
): Promise<void> {
	const currentFiles =
		existingFiles.length > 0 ? existingFiles : (getCurrentFileList()?.files ?? []);
	const action = { type: 'importPaths' as const, paths, existingFiles: currentFiles };
	const preparedEntry = enterImportAnalysisWorkflow(liveImportAnalysisWorkflowServices, action);
	if (!preparedEntry) {
		return;
	}
	await runImportAnalysisWorkflow(action, ImportAnalysisWorkflowLive, preparedEntry);
}

async function handleFileDrop(paths: string[], existingFiles: AudioFile[]): Promise<void> {
	await handleImportedAudioPaths(paths, existingFiles);
}

async function handleOpenedAudioFiles(existingFiles: AudioFile[] = []): Promise<boolean> {
	if (liveImportAnalysisWorkflowServices.isOrderLocked()) {
		liveImportAnalysisWorkflowServices.setFileImportError(importOrderLockedMessage());
		return false;
	}

	try {
		const paths = await tauriClient.takeOpenedAudioFiles();
		if (paths.length > 0) {
			await handleImportedAudioPaths(paths, existingFiles);
		}
		return true;
	} catch (cause) {
		console.error('Failed to import OS-opened audio files:', cause);
		setFileImportError('Failed to import opened audio files. Please try again.');
		return true;
	}
}

async function refreshSupportedAudioImportMetadata(): Promise<void> {
	try {
		const metadata = await tauriClient.getSupportedAudioImportMetadata();
		setFileImportSupportText(metadata.supportText);
	} catch (cause) {
		console.error('Failed to load supported audio import metadata:', cause);
	}
}
