import type { AudioFile, FileListInfo } from '../../types/audio';

export type InputSortDirection = 'none' | 'ascending' | 'descending';

export type SelectionModifiers = {
	readonly multi: boolean;
	readonly range: boolean;
};

export type InputSessionState = {
	readonly fileList: FileListInfo | null;
	readonly selectedIndices: ReadonlyArray<number>;
	readonly selectedAnchor: number;
	readonly sortDirection: InputSortDirection;
	readonly orderLocked: boolean;
	readonly errorMessage: string;
	readonly isDragOver: boolean;
	readonly supportText: string;
	readonly importOrdinalByPath: Readonly<Record<string, number>>;
	readonly nextImportOrdinal: number;
};

export type InputViewFile = AudioFile & {
	readonly index: number;
	readonly selected: boolean;
};

export type InputView = {
	readonly files: ReadonlyArray<InputViewFile>;
	readonly selectedIndices: ReadonlyArray<number>;
	readonly fileCount: number;
	readonly hasFiles: boolean;
	readonly orderLocked: boolean;
	readonly errorMessage: string;
	readonly isDragOver: boolean;
	readonly supportText: string;
	readonly sortDirection: InputSortDirection;
	readonly sortLabel: string;
	readonly orderDiffersFromImport: boolean;
};

export type ImportIntent =
	| { readonly type: 'pickFiles' }
	| { readonly type: 'pickFolder' }
	| { readonly type: 'importPaths'; readonly paths: ReadonlyArray<string> };

export const DEFAULT_SUPPORT_TEXT = 'Supports audio files';

export function emptyInputSession(): InputSessionState {
	return {
		fileList: null,
		selectedIndices: [],
		selectedAnchor: -1,
		sortDirection: 'none',
		orderLocked: false,
		errorMessage: '',
		isDragOver: false,
		supportText: DEFAULT_SUPPORT_TEXT,
		importOrdinalByPath: {},
		nextImportOrdinal: 0,
	};
}

export function fileIdentityKey(file: AudioFile): string {
	return file.inputId ?? file.path;
}
