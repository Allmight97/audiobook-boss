import { tauriClient } from '../../lib/tauri/client';
import type { AudioFile } from '../../types/audio';
import { isFileDropEvent } from '../../types/events';
import { applyCoverArtDrop } from '../coverArt';
import { getCurrentFileList } from '../fileList/state.svelte';
import { setFileImportDragOver } from './state.svelte';
import { enterImportAnalysisWorkflow, runImportAnalysisWorkflow } from './importAnalysisWorkflow';
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

async function handleFileDrop(paths: string[], existingFiles: AudioFile[]): Promise<void> {
	const currentFiles =
		existingFiles.length > 0 ? existingFiles : (getCurrentFileList()?.files ?? []);
	const action = { type: 'dropFiles' as const, paths, existingFiles: currentFiles };
	const preparedEntry = enterImportAnalysisWorkflow(liveImportAnalysisWorkflowServices, action);
	if (!preparedEntry) {
		return;
	}
	await runImportAnalysisWorkflow(action, ImportAnalysisWorkflowLive, preparedEntry);
}
