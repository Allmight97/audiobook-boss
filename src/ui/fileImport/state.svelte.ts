type FileImportUiState = {
	hasFiles: boolean;
	errorMessage: string;
	isDragOver: boolean;
	supportText: string;
};

export const fileImportUiState = $state<FileImportUiState>({
	hasFiles: false,
	errorMessage: '',
	isDragOver: false,
	supportText: 'Supports audio files',
});

export function setFileImportHasFiles(hasFiles: boolean): void {
	fileImportUiState.hasFiles = hasFiles;
}

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
