import { tauriClient } from '../../../lib/tauri/client';
import type { FileListInfo } from '../../../types/audio';
import { coverArtBytesToDataUrl } from '../../../lib/media/coverArtDataUrl';
import { boundProcessingInput } from '../bind';
import { fileListFromInput } from '../input';
import { setStatusPanelCoverArtDataUrl } from '../view';

export interface CoverArtTracker {
	syncForCurrentList(): Promise<void>;
	syncFromFileList(fileList: FileListInfo | null): Promise<void>;
	syncForFile(filePath: string): Promise<void>;
	reset(): void;
}

interface CoverArtTrackerDeps {
	getCurrentFileList?: () => FileListInfo | null;
	readCoverArtDataUrl?: (filePath: string) => Promise<string | null>;
	displayCoverArt?: (dataUrl: string) => void;
	resetArtThumbnail?: () => void;
	warn?: (message: string, error: unknown) => void;
}

function findFirstValidFilePath(fileList: FileListInfo | null): string | null {
	if (!fileList?.files.length) {
		return null;
	}

	return fileList.files.find((file) => file.isValid)?.path ?? null;
}

async function readCoverArtDataUrl(filePath: string): Promise<string | null> {
	const metadata = await tauriClient.readAudioMetadata(filePath);

	if (!metadata.cover_art || metadata.cover_art.length === 0) {
		return null;
	}

	return coverArtBytesToDataUrl(metadata.cover_art);
}

function defaultFileListReader(): FileListInfo | null {
	const view = boundProcessingInput()?.view();
	return view ? fileListFromInput(view) : null;
}

export function createCoverArtTracker(deps: CoverArtTrackerDeps = {}): CoverArtTracker {
	const readCurrentFileList = deps.getCurrentFileList ?? defaultFileListReader;
	const readCoverArt = deps.readCoverArtDataUrl ?? readCoverArtDataUrl;
	const displayCoverArt = deps.displayCoverArt ?? setStatusPanelCoverArtDataUrl;
	const resetArtThumbnail = deps.resetArtThumbnail ?? (() => setStatusPanelCoverArtDataUrl(null));
	const warn =
		deps.warn ??
		((message: string, error: unknown) => {
			console.warn(message, error);
		});

	// Keep the path sticky until explicit reset so repeated progress for the same
	// file does not churn metadata reads after a successful, empty, or failed load.
	let lastCoverArtPath: string | null = null;

	async function syncForFile(filePath: string): Promise<void> {
		if (lastCoverArtPath === filePath) {
			return;
		}

		lastCoverArtPath = filePath;

		try {
			const dataUrl = await readCoverArt(filePath);
			if (dataUrl) {
				displayCoverArt(dataUrl);
			} else {
				resetArtThumbnail();
			}
		} catch (error) {
			warn('Failed to load cover art for thumbnail:', error);
			resetArtThumbnail();
		}
	}

	async function syncFromFileList(fileList: FileListInfo | null): Promise<void> {
		const filePath = findFirstValidFilePath(fileList);
		if (!filePath) {
			resetArtThumbnail();
			return;
		}

		await syncForFile(filePath);
	}

	async function syncForCurrentList(): Promise<void> {
		await syncFromFileList(readCurrentFileList());
	}

	function reset(): void {
		lastCoverArtPath = null;
		resetArtThumbnail();
	}

	return {
		syncForCurrentList,
		syncFromFileList,
		syncForFile,
		reset,
	};
}
