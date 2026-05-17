import { tauriClient } from '../../lib/tauri/client';
import type { AudioFile, FileListInfo } from '../../types/audio';
import { isFileDropEvent } from '../../types/events';
import { applyCoverArtDrop } from '../coverArt';
import {
	appendFileList,
	persistPendingMetadataDraftsForCurrentSelection,
} from '../fileList/actions';
import { isOrderLocked } from '../fileList/state.svelte';
import { clearFileImportError, setFileImportDragOver, setFileImportError } from './state.svelte';
import {
	SUPPORTED_AUDIO_EXTENSIONS,
	SUPPORTED_AUDIO_FORMATS_TEXT,
	isSupportedAudioPath,
} from './supportedAudio';

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
	if (isOrderLocked()) {
		setFileImportError('Order locked while processing. Wait for completion to add files.');
		return;
	}

	try {
		const selected = await tauriClient.openFiles({
			filters: [
				{
					name: 'Audio Files',
					extensions: [...SUPPORTED_AUDIO_EXTENSIONS],
				},
			],
		});

		if (Array.isArray(selected) && selected.length > 0) {
			await processFilePaths(selected, existingFiles);
		}
	} catch (error) {
		setFileImportError(`Failed to open file dialog: ${error}`);
	}
}

async function handleFileDrop(paths: string[], existingFiles: AudioFile[]): Promise<void> {
	if (isOrderLocked()) {
		setFileImportError('Order locked while processing. Wait for completion to add files.');
		return;
	}

	const supportedPaths = filterSupportedFiles(paths);
	if (supportedPaths.length === 0) {
		setFileImportError(
			`No supported audio files dropped. Please use ${SUPPORTED_AUDIO_FORMATS_TEXT} files.`,
		);
		return;
	}

	await processFilePaths(supportedPaths, existingFiles);
}

function filterSupportedFiles(paths: string[]): string[] {
	return paths.filter((path) => isSupportedAudioPath(path));
}

async function processFilePaths(
	filePaths: string[],
	existingFiles: AudioFile[] = [],
): Promise<void> {
	if (filePaths.length === 0) return;

	try {
		const fileListInfo: FileListInfo = await tauriClient.analyzeAudioFiles(filePaths);
		const staged = await persistPendingMetadataDraftsForCurrentSelection();
		if (!staged) {
			setFileImportError('Fix metadata validation errors before adding files.');
			return;
		}
		appendFileList(fileListInfo, { existingFiles });
		clearFileImportError();
	} catch (error) {
		setFileImportError(`Failed to analyze files: ${error}`);
	}
}
