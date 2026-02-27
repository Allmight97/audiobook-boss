import { tauriClient } from '../lib/tauri/client';
import { isFileDropEvent } from '../types/events';
import {
	clearCoverArtMessageState,
	setCoverArtDataUrl,
	setCoverArtDragOver,
	setCoverArtHovered,
	setCoverArtLoading,
	setCoverArtMessage,
	setCoverArtUrlInputValue,
} from './coverArt/state.svelte';

// Global state for currently loaded cover art
let currentCoverArt: number[] | null = null;
// Tracks whether the user manually loaded custom cover art (preserved across file selection)
let hasCustomCoverArt = false;
// Tracks whether the user explicitly requested cover art removal in this session
let coverArtRemovalRequested = false;
let coverArtMessageTimeoutId: number | null = null;
let dragDropUnlisten: (() => void) | null = null;

/**
 * Initializes the cover art functionality
 */
export function initCoverArt(): void {
	if (dragDropUnlisten) {
		return;
	}

	// Handle global drag and drop from Tauri for cover art.
	const maybeUnlisten = tauriClient.listen('tauri://drag-drop', async (event) => {
		if (!isFileDropEvent(event.payload)) return;
		const { position, paths } = event.payload;

		const area = document.getElementById('cover-art-area');
		if (!area) return;
		setCoverArtDragOver(false);

		const rect = area.getBoundingClientRect();
		if (
			position.x < rect.left ||
			position.x > rect.right ||
			position.y < rect.top ||
			position.y > rect.bottom
		) {
			return;
		}

		const imageFile = paths.find((p) => /\.(jpg|jpeg|png|webp)$/i.test(p));
		if (imageFile) {
			await loadCoverArtFile(imageFile);
		}
	});

	if (!(maybeUnlisten instanceof Promise)) {
		return;
	}

	void maybeUnlisten.then((unlisten) => {
		dragDropUnlisten = unlisten;
	});
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
	currentCoverArt = imageData;
	hasCustomCoverArt = true;
	coverArtRemovalRequested = false;

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

function showCoverArtMessage(message: string, variant: 'error' | 'success'): void {
	setCoverArtMessage(message, variant);

	if (coverArtMessageTimeoutId !== null) {
		window.clearTimeout(coverArtMessageTimeoutId);
	}
	coverArtMessageTimeoutId = window.setTimeout(() => {
		clearCoverArtMessageState();
		coverArtMessageTimeoutId = null;
	}, 4000);
}

function clearCoverArtMessage(): void {
	clearCoverArtMessageState();
	if (coverArtMessageTimeoutId !== null) {
		window.clearTimeout(coverArtMessageTimeoutId);
		coverArtMessageTimeoutId = null;
	}
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
	return currentCoverArt;
}

export function getHasCustomCoverArt(): boolean {
	return hasCustomCoverArt;
}

export function isCoverArtRemovalRequested(): boolean {
	return coverArtRemovalRequested;
}

export function setCoverArt(coverArtBytes: number[] | null): void {
	currentCoverArt = coverArtBytes;
	if (coverArtBytes && coverArtBytes.length > 0) {
		coverArtRemovalRequested = false;
	}
	displayCoverArt(coverArtBytes);
}

export function setCustomCoverArt(coverArtBytes: number[] | null): void {
	currentCoverArt = coverArtBytes;
	hasCustomCoverArt = Boolean(coverArtBytes && coverArtBytes.length > 0);
	coverArtRemovalRequested = false;
	displayCoverArt(coverArtBytes);
}

export function clearCoverArt(options?: { markRemoval?: boolean }): void {
	const markRemoval = options?.markRemoval ?? false;
	currentCoverArt = null;
	displayCoverArt(null);
	coverArtRemovalRequested = markRemoval;
	hasCustomCoverArt = false;
	setCoverArtUrlInputValue('');
	clearCoverArtMessage();
	setCoverArtDragOver(false);
	setCoverArtHovered(false);
}
