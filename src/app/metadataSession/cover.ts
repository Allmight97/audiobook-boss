import type { MetadataIntentPatch } from '../../types/metadataIntent';

export type CoverArtMessage =
	| { readonly kind: 'hidden' }
	| { readonly kind: 'error'; readonly text: string }
	| { readonly kind: 'success'; readonly text: string };

export type CoverUiState = {
	readonly imageDataUrl: string | null;
	readonly isLoading: boolean;
	readonly message: CoverArtMessage;
	readonly isHovered: boolean;
	readonly isDragOver: boolean;
	readonly urlInputValue: string;
	readonly hasCustomCoverArt: boolean;
	readonly coverArtRemovalRequested: boolean;
	readonly currentCoverHandle: string | null;
};

export function createEmptyCoverUiState(): CoverUiState {
	return {
		imageDataUrl: null,
		isLoading: false,
		message: { kind: 'hidden' },
		isHovered: false,
		isDragOver: false,
		urlInputValue: '',
		hasCustomCoverArt: false,
		coverArtRemovalRequested: false,
		currentCoverHandle: null,
	};
}

export const COVER_ART_IMAGE_EXTENSION_HINTS = ['jpg', 'jpeg', 'png', 'webp'] as const;
export const COVER_ART_IMAGE_EXTENSION_HINT_PATTERN = new RegExp(
	`\\.(${COVER_ART_IMAGE_EXTENSION_HINTS.join('|')})$`,
	'i',
);

export function parseCoverArtUrl(raw: string): URL | null {
	try {
		return new URL(raw);
	} catch {
		return null;
	}
}

export function formatCoverArtError(message: string, fallback: string): string {
	const raw = message || fallback;
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

export function coverIntentFromUi(cover: CoverUiState): MetadataIntentPatch {
	if (cover.coverArtRemovalRequested) {
		return { cover_art: { op: 'clear' } };
	}
	if (cover.hasCustomCoverArt && cover.currentCoverHandle) {
		return { cover_art: { op: 'set', value: cover.currentCoverHandle } };
	}
	return {};
}
