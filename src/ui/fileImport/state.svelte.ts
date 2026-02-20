type FileImportUiState = {
	hasFiles: boolean;
	errorMessage: string;
};

export const fileImportUiState = $state<FileImportUiState>({
	hasFiles: false,
	errorMessage: '',
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
