type FileImportUiState = {
	errorMessage: string;
	isDragOver: boolean;
	supportText: string;
};

export const fileImportUiState = $state<FileImportUiState>({
	errorMessage: '',
	isDragOver: false,
	supportText: 'Supports audio files',
});

export function setFileImportError(message: string): void {
	fileImportUiState.errorMessage = message;
}

export function clearFileImportError(): void {
	fileImportUiState.errorMessage = '';
}

export function setFileImportDragOver(isDragOver: boolean): void {
	fileImportUiState.isDragOver = isDragOver;
}

export function setFileImportSupportText(supportText: string): void {
	fileImportUiState.supportText = supportText;
}
