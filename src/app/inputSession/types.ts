import type { AudioFile, AudioHandling, FileListInfo } from '../../types/audio';

export type InputSortDirection = 'none' | 'ascending' | 'descending';

export type SelectionModifiers = {
	readonly multi: boolean;
	readonly range: boolean;
};

export type InputSessionState = {
	// Each visible entry anchors a title's metadata. Ordered sources live separately.
	readonly fileList: FileListInfo | null;
	readonly titleSourcesByIdentity: Readonly<Record<string, ReadonlyArray<AudioFile>>>;
	readonly audioChoiceRequired: ReadonlyArray<string>;
	readonly selectedIndices: ReadonlyArray<number>;
	readonly selectedAnchor: number;
	readonly sortDirection: InputSortDirection;
	readonly orderLocked: boolean;
	readonly errorMessage: string;
	readonly isDragOver: boolean;
	readonly supportText: string;
	readonly importOrdinalByPath: Readonly<Record<string, number>>;
	readonly nextImportOrdinal: number;
	readonly audioHandlingByIdentity: Readonly<Record<string, AudioHandling>>;
};

export type InputView = {
	readonly files: ReadonlyArray<AudioFile>;
	readonly sourceFiles: ReadonlyArray<AudioFile>;
	readonly selectedSourceFiles: ReadonlyArray<AudioFile>;
	readonly selectedIndices: ReadonlyArray<number>;
	readonly selectedAnchor: number;
	readonly fileCount: number;
	readonly hasFiles: boolean;
	readonly orderLocked: boolean;
	readonly errorMessage: string;
	readonly isDragOver: boolean;
	readonly supportText: string;
	readonly sortDirection: InputSortDirection;
	readonly sortLabel: string;
	readonly orderDiffersFromImport: boolean;
	readonly showSortButton: boolean;
	readonly showClearButton: boolean;
	readonly showRestoreImportOrder: boolean;
	readonly totalDurationSeconds: number;
};

export type ImportIntent =
	| { readonly type: 'pickFiles' }
	| { readonly type: 'pickFolder' }
	| { readonly type: 'importPaths'; readonly paths: ReadonlyArray<string> }
	| { readonly type: 'drainOpened' };

export const DEFAULT_SUPPORT_TEXT = 'Supports audio files';

export function emptyInputSession(): InputSessionState {
	return {
		fileList: null,
		titleSourcesByIdentity: {},
		audioChoiceRequired: [],
		selectedIndices: [],
		selectedAnchor: -1,
		sortDirection: 'none',
		orderLocked: false,
		errorMessage: '',
		isDragOver: false,
		supportText: DEFAULT_SUPPORT_TEXT,
		importOrdinalByPath: {},
		nextImportOrdinal: 0,
		audioHandlingByIdentity: {},
	};
}

export function fileIdentityKey(file: AudioFile): string {
	return file.inputId ?? file.path;
}

export function orderDiffersFromImport(
	files: ReadonlyArray<AudioFile>,
	importOrdinalByPath: Readonly<Record<string, number>>,
): boolean {
	if (files.length <= 1) return false;
	let previous = -1;
	for (const file of files) {
		const ordinal = importOrdinalByPath[file.path];
		if (ordinal === undefined) return false;
		if (ordinal < previous) return true;
		previous = ordinal;
	}
	return false;
}
