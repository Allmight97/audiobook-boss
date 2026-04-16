import { tauriClient } from '../lib/tauri/client';
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
} from './coverArt/state.svelte';

let coverArtMessageTimeoutId: number | null = null;

/**
 * Initializes the cover art functionality
 */
export function initCoverArt(): void {
	// no-op lifecycle hook kept for init ordering and tests
	// drag/drop routing is handled by fileImport handlers now.
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
		const selectedFile = await tauriClient.open({
			multiple: false,
			directory: false,
			title: 'Select Cover Art Image',
			filters: [
				{
					name: 'Image Files',
					extensions: ['jpg', 'jpeg', 'png', 'webp'],
				},
			],
		});

		if (!selectedFile || typeof selectedFile !== 'string') {
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
	setCoverArtSession(imageData);
	setHasCustomCoverArt(true);
	setCoverArtRemovalRequested(false);

	displayCoverArt(imageData);
}

function coverArtBytesToDataUrl(coverArtBytes: number[]): string {
	const uint8Array = new Uint8Array(coverArtBytes);
	const base64String = btoa(String.fromCharCode(...uint8Array));

	let mimeType = 'image/jpeg';
	if (coverArtBytes.length >= 12) {
		if (coverArtBytes[0] === 0x89 && coverArtBytes[1] === 0x50) {
			mimeType = 'image/png';
		} else if (
			coverArtBytes[0] === 0x52 &&
			coverArtBytes[1] === 0x49 &&
			coverArtBytes[2] === 0x46 &&
			coverArtBytes[3] === 0x46 &&
			coverArtBytes[8] === 0x57 &&
			coverArtBytes[9] === 0x45 &&
			coverArtBytes[10] === 0x42 &&
			coverArtBytes[11] === 0x50
		) {
			mimeType = 'image/webp';
		}
	} else if (coverArtBytes.length >= 2 && coverArtBytes[0] === 0x89 && coverArtBytes[1] === 0x50) {
		mimeType = 'image/png';
	}

	return `data:${mimeType};base64,${base64String}`;
}

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
	const imageFile = paths.find((path) => /\.(jpg|jpeg|png|webp)$/i.test(path));
	if (!imageFile) {
		return false;
	}

	await loadCoverArtFile(imageFile);
	return true;
}

function formatCoverArtError(error: unknown, fallback: string): string {
	const raw = error instanceof Error ? error.message : typeof error === 'string' ? error : fallback;
	if (/status 403/i.test(raw) || /403 Forbidden/i.test(raw)) {
		return 'That URL blocked the image request (Error 403) - download the image and Load Cover Art from file.';
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

// Global Exports
export function getCurrentCoverArt(): number[] | null {
	return coverArtSessionState.currentCoverArt;
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
	setCoverArtSession(coverArtBytes);
	setHasCustomCoverArt(Boolean(coverArtBytes && coverArtBytes.length > 0));
	setCoverArtRemovalRequested(false);
	displayCoverArt(coverArtBytes);
}

export function clearCoverArt(options?: { markRemoval?: boolean }): void {
	const markRemoval = options?.markRemoval ?? false;
	clearCoverArtSession();
	displayCoverArt(null);
	setCoverArtRemovalRequested(markRemoval);
	setHasCustomCoverArt(false);
	setCoverArtUrlInputValue('');
	clearCoverArtMessage();
	setCoverArtDragOver(false);
	setCoverArtHovered(false);
}
