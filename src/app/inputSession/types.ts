import type { AudioFile } from '../../types/audio';

export type InputSortDirection = 'none' | 'ascending' | 'descending';

export type SelectionModifiers = {
	readonly multi: boolean;
	readonly range: boolean;
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
	readonly totalSizeBytes: number;
	readonly canGroup: boolean;
	readonly totalDurationSeconds: number;
};

export type ImportIntent =
	| { readonly type: 'pickFiles' }
	| { readonly type: 'pickFolder' }
	| { readonly type: 'importPaths'; readonly paths: ReadonlyArray<string> };

export function fileIdentityKey(file: AudioFile): string {
	return file.inputId ?? file.path;
}
