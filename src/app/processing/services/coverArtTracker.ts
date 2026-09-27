import { tauriClient } from '../../../lib/tauri/client';
import type { AudioFile } from '../../../types/audio';
import { coverArtBytesToDataUrl } from '../../../lib/media/coverArtDataUrl';

export interface CoverArtTracker {
	syncForCurrentList(): Promise<void>;
	syncForFile(filePath: string): Promise<void>;
	reset(): void;
}

interface CoverArtTrackerDeps {
	validTitles?: () => ReadonlyArray<AudioFile>;
	readCoverArtDataUrl?: (filePath: string) => Promise<string | null>;
	displayCoverArt?: (dataUrl: string) => void;
	resetArtThumbnail?: () => void;
	warn?: (message: string, error: unknown) => void;
}

async function readCoverArtDataUrl(filePath: string): Promise<string | null> {
	const metadata = await tauriClient.readAudioMetadata(filePath);

	if (!metadata.cover_art || metadata.cover_art.length === 0) {
		return null;
	}

	return coverArtBytesToDataUrl(metadata.cover_art);
}

export function createCoverArtTracker(deps: CoverArtTrackerDeps = {}): CoverArtTracker {
	const readValidTitles = deps.validTitles ?? (() => []);
	const readCoverArt = deps.readCoverArtDataUrl ?? readCoverArtDataUrl;
	const displayCoverArt = deps.displayCoverArt ?? (() => undefined);
	const resetArtThumbnail = deps.resetArtThumbnail ?? (() => undefined);
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

	async function syncForCurrentList(): Promise<void> {
		const filePath = readValidTitles()[0]?.path;
		if (!filePath) {
			resetArtThumbnail();
			return;
		}

		await syncForFile(filePath);
	}

	function reset(): void {
		lastCoverArtPath = null;
		resetArtThumbnail();
	}

	return {
		syncForCurrentList,
		syncForFile,
		reset,
	};
}
