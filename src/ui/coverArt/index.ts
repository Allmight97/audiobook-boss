import { coverArtBytesToDataUrl } from '../../lib/media/coverArtDataUrl';
import { tauriClient } from '../../lib/tauri/client';
import { toUserMessage } from '../../lib/tauri/appError';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import { getJobType } from '../jobControls';
import { getCurrentFileList, getSelectedFiles } from '../fileList';
import { stageMetadataIntentPatch } from '../metadataSession';
import {
	effectiveCoverForFile,
	resolveCoverDisplayPath,
	resolveCoverOwnerPaths,
} from './coverOwner';
import {
	clearCoverArtMessageState,
	clearCoverArtSession,
	setCoverArtRemovalRequested,
	setHasCustomCoverArt,
	setCoverArtSession,
	setCoverArtDataUrl,
	setCoverArtDragOver,
	setCoverArtHovered,
	setCoverArtLoading,
	setCoverArtMessage,
	setCoverArtUrlInputValue,
	coverArtSessionState,
} from './state.svelte';

let coverArtMessageTimeoutId: number | null = null;

function readJobType() {
	return getJobType();
}

export const COVER_ART_IMAGE_EXTENSION_HINTS = ['jpg', 'jpeg', 'png', 'webp'] as const;
const COVER_ART_IMAGE_EXTENSION_HINT_PATTERN = new RegExp(
	`\\.(${COVER_ART_IMAGE_EXTENSION_HINTS.join('|')})$`,
	'i',
);

function buildCoverArtIntentPatch(
	coverArtBytes: number[] | null,
	markRemoval: boolean,
): MetadataIntentPatch {
	if (markRemoval || !coverArtBytes || coverArtBytes.length === 0) {
		return { cover_art: { op: 'clear' } };
	}
	return { cover_art: { op: 'set', value: [...coverArtBytes] } };
}

function commitCoverArtToOwners(coverArtBytes: number[] | null, markRemoval: boolean): boolean {
	const ownerPaths = resolveCoverOwnerPaths(
		readJobType(),
		getCurrentFileList(),
		getSelectedFiles(),
	);
	if (ownerPaths.length === 0) {
		return false;
	}

	const intentPatch = buildCoverArtIntentPatch(coverArtBytes, markRemoval);
	for (const filePath of ownerPaths) {
		stageMetadataIntentPatch(filePath, intentPatch);
	}
	return true;
}

export function refreshCoverArtDisplay(): void {
	const displayPath = resolveCoverDisplayPath(
		readJobType(),
		getCurrentFileList(),
		getSelectedFiles(),
	);
	if (!displayPath) {
		displayCoverArt(null);
		return;
	}
	displayCoverArt(effectiveCoverForFile(displayPath));
}

/**
 * Handles the Clear Cover Art action
 */
export function onClearCoverArt(): void {
	clearCoverArt({ markRemoval: true });
	console.log('Cover art cleared');
}

/**
 * Handles the Click-to-Load action
 */
export async function onLoadCoverArtFromFilePicker(): Promise<void> {
	try {
		const selectedFile = await tauriClient.openFile({
			title: 'Select Cover Art Image',
			filters: [
				{
					name: 'Image Files',
					extensions: [...COVER_ART_IMAGE_EXTENSION_HINTS],
				},
			],
		});

		if (!selectedFile) {
			return;
		}

		await loadCoverArtFile(selectedFile);
	} catch (error) {
		console.error('Failed to open file dialog:', error);
	}
}

export async function onLoadCoverArtFromInput(rawInput: string): Promise<string | null> {
	const raw = rawInput.trim();
	if (!raw) {
		showCoverArtMessage('Paste an image URL first.', 'error');
		return null;
	}

	const parsed = parseCoverArtUrl(raw);
	if (!parsed) {
		showCoverArtMessage('Invalid URL format.', 'error');
		return null;
	}

	if (parsed.protocol !== 'https:') {
		showCoverArtMessage('Only HTTPS URLs are supported.', 'error');
		return null;
	}

	const normalized = parsed.toString();
	setCoverArtUrlInputValue(normalized);
	await loadCoverArtFromUrl(normalized);
	return normalized;
}

/**
 * Loads cover art from a specific file path
 */
async function loadCoverArtFile(filePath: string): Promise<void> {
	try {
		const imageData = await tauriClient.loadCoverArtFile(filePath);

		applyLoadedCoverArt(imageData);

		console.log('Cover art loaded:', filePath);
	} catch (error) {
		console.error('Failed to load cover art file:', error);
		showCoverArtError(formatCoverArtError(error, 'Unknown error'));
	}
}

async function loadCoverArtFromUrl(url: string): Promise<void> {
	try {
		setCoverArtLoading(true);
		clearCoverArtMessage();
		const imageData = await tauriClient.loadCoverArtFromUrl(url);

		applyLoadedCoverArt(imageData);
		showCoverArtMessage('Cover art loaded from URL.', 'success');
	} catch (error) {
		console.error('Failed to load cover art URL:', error);
		showCoverArtError(formatCoverArtError(error, 'Unable to load image.'));
	} finally {
		setCoverArtLoading(false);
	}
}

function applyLoadedCoverArt(imageData: number[]): void {
	setCustomCoverArt(imageData);
}

export { coverArtBytesToDataUrl };

/**
 * Displays cover art in the UI
 */
export function displayCoverArt(coverArtBytes: number[] | null): void {
	if (coverArtBytes && coverArtBytes.length > 0) {
		setCoverArtDataUrl(coverArtBytesToDataUrl(coverArtBytes));
		return;
	}
	setCoverArtDataUrl(null);
}

function showCoverArtError(message: string): void {
	console.error('Cover Art Error:', message);
	showCoverArtMessage(message, 'error');
}

export function showCoverArtMessage(message: string, variant: 'error' | 'success'): void {
	setCoverArtMessage(message, variant);

	if (coverArtMessageTimeoutId !== null) {
		window.clearTimeout(coverArtMessageTimeoutId);
	}
	coverArtMessageTimeoutId = window.setTimeout(() => {
		clearCoverArtMessageState();
		coverArtMessageTimeoutId = null;
	}, 4000);
}

export function clearCoverArtMessage(): void {
	clearCoverArtMessageState();
	if (coverArtMessageTimeoutId !== null) {
		window.clearTimeout(coverArtMessageTimeoutId);
		coverArtMessageTimeoutId = null;
	}
}

/**
 * Attempts to route a drag/drop payload through cover-art loading.
 * Returns true if an image file was consumed.
 */
export async function applyCoverArtDrop(paths: string[]): Promise<boolean> {
	const imageFile = paths.find((path) => COVER_ART_IMAGE_EXTENSION_HINT_PATTERN.test(path));
	if (!imageFile) {
		return false;
	}

	await loadCoverArtFile(imageFile);
	return true;
}

function formatCoverArtError(error: unknown, fallback: string): string {
	const raw = toUserMessage(error, { fallback });
	if (/status 403/i.test(raw) || /403 Forbidden/i.test(raw)) {
		return 'That URL blocked the image request (Error 403) - download the image and Load Cover Art from file.';
	}
	if (/unsupported image format|file has no extension/i.test(raw)) {
		return 'Unsupported image format. Use JPEG, PNG, or WebP.';
	}
	if (/only https urls? are supported/i.test(raw)) {
		return 'Only HTTPS URLs are supported.';
	}
	if (/invalid url|url must include a host/i.test(raw)) {
		return 'Invalid image URL.';
	}
	return raw;
}

function parseCoverArtUrl(raw: string): URL | null {
	try {
		return new URL(raw);
	} catch {
		return null;
	}
}

export function getCurrentCoverArt(): number[] | null {
	const displayPath = resolveCoverDisplayPath(
		readJobType(),
		getCurrentFileList(),
		getSelectedFiles(),
	);
	if (!displayPath) {
		return null;
	}
	return effectiveCoverForFile(displayPath);
}

export function getHasCustomCoverArt(): boolean {
	return coverArtSessionState.hasCustomCoverArt;
}

export function isCoverArtRemovalRequested(): boolean {
	return coverArtSessionState.coverArtRemovalRequested;
}

export function setCoverArt(coverArtBytes: number[] | null): void {
	setCoverArtSession(coverArtBytes);
	if (coverArtBytes && coverArtBytes.length > 0) {
		setCoverArtRemovalRequested(false);
	}
	displayCoverArt(coverArtBytes);
}

export function setCustomCoverArt(coverArtBytes: number[] | null): void {
	if (!coverArtBytes || coverArtBytes.length === 0) {
		clearCoverArt({ markRemoval: true });
		return;
	}

	const committed = commitCoverArtToOwners(coverArtBytes, false);
	if (!committed) {
		clearCoverArtSession();
		setHasCustomCoverArt(false);
		setCoverArtRemovalRequested(false);
		refreshCoverArtDisplay();
		return;
	}
	setCoverArtSession(coverArtBytes);
	setHasCustomCoverArt(true);
	setCoverArtRemovalRequested(false);
	refreshCoverArtDisplay();
}

export function clearCoverArt(options?: { markRemoval?: boolean }): void {
	const markRemoval = options?.markRemoval ?? false;

	if (markRemoval) {
		commitCoverArtToOwners(null, markRemoval);
	}

	clearCoverArtSession();
	setCoverArtRemovalRequested(markRemoval);
	setHasCustomCoverArt(false);
	setCoverArtUrlInputValue('');
	clearCoverArtMessage();
	setCoverArtDragOver(false);
	setCoverArtHovered(false);
	refreshCoverArtDisplay();
}
