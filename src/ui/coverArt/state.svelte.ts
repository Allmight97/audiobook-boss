type CoverArtMessageVariant = 'error' | 'success' | null;

type CoverArtSessionState = {
	currentCoverArt: number[] | null;
	hasCustomCoverArt: boolean;
	coverArtRemovalRequested: boolean;
};

type CoverArtUiState = {
	imageDataUrl: string | null;
	isLoading: boolean;
	messageText: string;
	messageVariant: CoverArtMessageVariant;
	messageVisible: boolean;
	isHovered: boolean;
	isDragOver: boolean;
	urlInputValue: string;
};

export const coverArtSessionState = $state<CoverArtSessionState>({
	currentCoverArt: null,
	hasCustomCoverArt: false,
	coverArtRemovalRequested: false,
});

export const coverArtUiState = $state<CoverArtUiState>({
	imageDataUrl: null,
	isLoading: false,
	messageText: '',
	messageVariant: null,
	messageVisible: false,
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

export function setCoverArtMessage(
	message: string,
	variant: Exclude<CoverArtMessageVariant, null>,
): void {
	coverArtUiState.messageText = message;
	coverArtUiState.messageVariant = variant;
	coverArtUiState.messageVisible = true;
}

export function clearCoverArtMessageState(): void {
	coverArtUiState.messageText = '';
	coverArtUiState.messageVariant = null;
	coverArtUiState.messageVisible = false;
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
}

export function clearCoverArtSession(): void {
	coverArtSessionState.currentCoverArt = null;
}

export function setHasCustomCoverArt(hasCustom: boolean): void {
	coverArtSessionState.hasCustomCoverArt = hasCustom;
}

export function setCoverArtRemovalRequested(markRemoval: boolean): void {
	coverArtSessionState.coverArtRemovalRequested = markRemoval;
}
