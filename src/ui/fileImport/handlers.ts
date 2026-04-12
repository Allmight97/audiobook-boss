import { tauriClient } from '../../lib/tauri/client';
import type { FileListInfo } from '../../types/audio';
import { isFileDropEvent } from '../../types/events';
import { applyCoverArtDrop } from '../coverArt';
import { displayFileList } from '../fileList';
import { isOrderLocked } from '../fileList/state';
import { clearFileImportError, setFileImportError } from './state.svelte';
import {
	SUPPORTED_AUDIO_EXTENSIONS,
	SUPPORTED_AUDIO_FORMATS_TEXT,
	isSupportedAudioPath,
} from './supportedAudio';

export interface DragDropContext {
	getDropZoneHeader: () => HTMLElement | null;
	getCoverArtArea: () => HTMLElement | null;
	getFileManagementContainer: () => HTMLElement | null;
}

type Unlisten = () => void;

export function attachTauriDragHandlers(context: DragDropContext): Unlisten {
	const { getDropZoneHeader, getCoverArtArea, getFileManagementContainer } = context;
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
		getDropZoneHeader()?.classList.remove('drag-over');
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
			await handleFileDrop(event.payload.paths);
		}
	};

	captureUnlistener(tauriClient.listen('tauri://drag-drop', dragDropHandler));
	captureUnlistener(
		tauriClient.listen('tauri://drag-enter', () => {
			getDropZoneHeader()?.classList.add('drag-over');
		}),
	);
	captureUnlistener(
		tauriClient.listen('tauri://drag-leave', () => {
			getDropZoneHeader()?.classList.remove('drag-over');
		}),
	);

	return () => {
		isDisposed = true;
		for (const unlisten of unlisteners.splice(0, unlisteners.length)) {
			unlisten();
		}
	};
}

export async function handleClickToSelect(): Promise<void> {
	if (isOrderLocked()) {
		setFileImportError('Order locked while processing. Wait for completion to add files.');
		return;
	}

	try {
		const selected = await tauriClient.open({
			multiple: true,
			directory: false,
			filters: [
				{
					name: 'Audio Files',
					extensions: [...SUPPORTED_AUDIO_EXTENSIONS],
				},
			],
		});

		if (Array.isArray(selected) && selected.length > 0) {
			await processFilePaths(selected);
		} else if (typeof selected === 'string') {
			await processFilePaths([selected]);
		}
	} catch (error) {
		setFileImportError(`Failed to tauriClient.open file dialog: ${error}`);
	}
}

async function handleFileDrop(paths: string[]): Promise<void> {
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

	await processFilePaths(supportedPaths);
}

function filterSupportedFiles(paths: string[]): string[] {
	return paths.filter((path) => isSupportedAudioPath(path));
}

async function processFilePaths(filePaths: string[]): Promise<void> {
	if (filePaths.length === 0) return;

	try {
		const fileListInfo: FileListInfo = await tauriClient.analyzeAudioFiles(filePaths);
		displayFileList(fileListInfo);
		clearFileImportError();
	} catch (error) {
		setFileImportError(`Failed to analyze files: ${error}`);
	}
}
