import { toUserMessage } from '../../lib/tauri/appError';
import type { CoverNotice } from '../../types/session';

export type CoverArtMessage =
	| { readonly kind: 'hidden' }
	| { readonly kind: 'error'; readonly text: string }
	| { readonly kind: 'success'; readonly text: string };

export type CoverUiState = {
	/** The address of the cover on screen; absent when there is none. */
	readonly imageSrc: string | null;
	readonly isLoading: boolean;
	readonly message: CoverArtMessage;
	readonly isHovered: boolean;
	readonly isDragOver: boolean;
	readonly urlInputValue: string;
	readonly hasCustomCoverArt: boolean;
	readonly coverArtRemovalRequested: boolean;
};

export const HIDDEN_COVER_MESSAGE: CoverArtMessage = { kind: 'hidden' };

/** How long a cover message stays up before it hides itself. */
export const COVER_MESSAGE_MS = 4000;

export function coverNoticeMessage(notice: CoverNotice | null): CoverArtMessage {
	switch (notice?.kind) {
		case undefined:
			return HIDDEN_COVER_MESSAGE;
		case 'urlRequired':
			return { kind: 'error', text: 'Paste an image URL first.' };
		case 'loadedFromUrl':
			return { kind: 'success', text: 'Cover art loaded from URL.' };
		case 'loadFailed':
			return {
				kind: 'error',
				text: toUserMessage(notice.error, { fallback: 'Unable to load cover art.' }),
			};
	}
}

export const COVER_ART_IMAGE_EXTENSION_HINTS = ['jpg', 'jpeg', 'png', 'webp'] as const;
