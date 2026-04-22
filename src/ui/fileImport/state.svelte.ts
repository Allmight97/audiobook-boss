type FileImportUiState = {
	hasFiles: boolean;
	errorMessage: string;
	isDragOver: boolean;
};

export const fileImportUiState = $state<FileImportUiState>({
	hasFiles: false,
	errorMessage: '',
	isDragOver: false,
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
