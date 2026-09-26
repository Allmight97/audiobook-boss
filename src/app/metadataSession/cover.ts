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
	readonly currentCoverArt: number[] | null;
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
		currentCoverArt: null,
	};
}

export const COVER_ART_IMAGE_EXTENSION_HINTS = ['jpg', 'jpeg', 'png', 'webp'] as const;
export const COVER_ART_IMAGE_EXTENSION_HINT_PATTERN = new RegExp(
	`\\.(${COVER_ART_IMAGE_EXTENSION_HINTS.join('|')})$`,
	'i',
);
