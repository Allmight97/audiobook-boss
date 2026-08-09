/**
 * Cover-art inline message. Modeled as a discriminated union so impossible
 * states (e.g. `messageVisible: false` paired with lingering `messageText`)
 * are unrepresentable. Only `error` and `success` variants carry text — the
 * hidden variant has no text at all.
 */
export type CoverArtMessage =
	| { kind: 'hidden' }
	| { kind: 'error'; text: string }
	| { kind: 'success'; text: string };

type CoverArtSessionState = {
	currentCoverArt: number[] | null;
	hasCustomCoverArt: boolean;
	coverArtRemovalRequested: boolean;
};

type CoverArtUiState = {
	imageDataUrl: string | null;
	isLoading: boolean;
	message: CoverArtMessage;
	isHovered: boolean;
	isDragOver: boolean;
	urlInputValue: string;
};

export const coverArtSessionState = $state<CoverArtSessionState>({
	currentCoverArt: null,
	hasCustomCoverArt: false,
	coverArtRemovalRequested: false,
});

let coverArtSessionRevision = 0;

function bumpCoverArtSessionRevision(): void {
	coverArtSessionRevision += 1;
}

export function getCoverArtSessionRevision(): number {
	return coverArtSessionRevision;
}

export const coverArtUiState = $state<CoverArtUiState>({
	imageDataUrl: null,
	isLoading: false,
	message: { kind: 'hidden' },
	isHovered: false,
	isDragOver: false,
	urlInputValue: '',
});

export function setCoverArtDataUrl(dataUrl: string | null): void {
	coverArtUiState.imageDataUrl = dataUrl;
}

export function setCoverArtLoading(isLoading: boolean): void {
	coverArtUiState.isLoading = isLoading;
}

export function setCoverArtMessage(text: string, kind: 'error' | 'success'): void {
	coverArtUiState.message = { kind, text };
}

export function clearCoverArtMessageState(): void {
	coverArtUiState.message = { kind: 'hidden' };
}

export function setCoverArtHovered(isHovered: boolean): void {
	coverArtUiState.isHovered = isHovered;
}

export function setCoverArtDragOver(isDragOver: boolean): void {
	coverArtUiState.isDragOver = isDragOver;
}

export function setCoverArtUrlInputValue(value: string): void {
	coverArtUiState.urlInputValue = value;
}

export function setCoverArtSession(coverArtBytes: number[] | null): void {
	coverArtSessionState.currentCoverArt = coverArtBytes;
	bumpCoverArtSessionRevision();
}

export function clearCoverArtSession(): void {
	coverArtSessionState.currentCoverArt = null;
	bumpCoverArtSessionRevision();
}

export function setHasCustomCoverArt(hasCustom: boolean): void {
	coverArtSessionState.hasCustomCoverArt = hasCustom;
	bumpCoverArtSessionRevision();
}

export function setCoverArtRemovalRequested(markRemoval: boolean): void {
	coverArtSessionState.coverArtRemovalRequested = markRemoval;
	bumpCoverArtSessionRevision();
}
