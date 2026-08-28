import { createRoot, createSignal, flush } from 'solid-js';
import { pathBasename } from '../../../lib/path/basename';
import type { AudioFile, FileListInfo } from '../../../types/audio';
import { clearFileListCoverThumbnails, removeFileListCoverThumbnail } from './thumbnails';

export type FileListSortDirection = 'none' | 'ascending' | 'descending';

export type FileListSession = {
	currentFileList: FileListInfo | null;
	selectedFileIndex: number;
	selectedFileIndices: ReadonlySet<number>;
	sortDirection: FileListSortDirection;
	orderLocked: boolean;
};

export type SelectionModifiers = { multi: boolean; range: boolean };

const INITIAL: FileListSession = {
	currentFileList: null,
	selectedFileIndex: -1,
	selectedFileIndices: new Set<number>(),
	sortDirection: 'none',
	orderLocked: false,
};

const importOrdinalByPath = new Map<string, number>();
let nextImportOrdinal = 0;

const graph = createRoot(() => {
	const [read, set] = createSignal<FileListSession>({
		...INITIAL,
		selectedFileIndices: new Set<number>(),
	});
	return {
		read,
		write(next: FileListSession) {
			set(next);
			flush();
		},
	};
});

export const fileListSession = graph.read;

function write(next: FileListSession): void {
	graph.write(next);
}

export function getImportOrdinal(path: string): number | undefined {
	return importOrdinalByPath.get(path);
}

export function fileIdentityKey(file: AudioFile): string {
	return file.inputId ?? file.path;
}

function recordImportOrder(files: readonly AudioFile[]): void {
	for (const file of files) {
		if (!importOrdinalByPath.has(file.path)) {
			importOrdinalByPath.set(file.path, nextImportOrdinal++);
		}
	}
}

function replaceFiles(session: FileListSession, nextFiles: AudioFile[]): FileListSession {
	const fileList = session.currentFileList;
	if (!fileList) return session;
	const validCount = nextFiles.filter((file) => file.isValid).length;
	return {
		...session,
		currentFileList: {
			...fileList,
			files: nextFiles,
			validCount,
			invalidCount: nextFiles.length - validCount,
		},
	};
}

function ensureAnchor(
	selected: ReadonlySet<number>,
	anchor: number,
): Pick<FileListSession, 'selectedFileIndex' | 'selectedFileIndices'> {
	if (selected.size === 0) return { selectedFileIndex: -1, selectedFileIndices: selected };
	if (anchor >= 0 && selected.has(anchor)) {
		return { selectedFileIndex: anchor, selectedFileIndices: selected };
	}
	const sorted = [...selected].sort((a, b) => a - b);
	return { selectedFileIndex: sorted[sorted.length - 1] ?? -1, selectedFileIndices: selected };
}

function remapSelection(
	files: readonly AudioFile[],
	nextFiles: readonly AudioFile[],
	selected: ReadonlySet<number>,
	anchor: number,
): Pick<FileListSession, 'selectedFileIndex' | 'selectedFileIndices'> {
	const keys = new Set(
		[...selected]
			.map((index) => files[index])
			.filter((file): file is AudioFile => Boolean(file))
			.map(fileIdentityKey),
	);
	const anchorKey = files[anchor] ? fileIdentityKey(files[anchor]) : null;
	return {
		selectedFileIndices: new Set(
			nextFiles.flatMap((file, index) => (keys.has(fileIdentityKey(file)) ? [index] : [])),
		),
		selectedFileIndex: anchorKey
			? nextFiles.findIndex((file) => fileIdentityKey(file) === anchorKey)
			: -1,
	};
}

function mapIndexForMove(index: number, fromIndex: number, toIndex: number): number {
	if (index === fromIndex) return toIndex;
	if (fromIndex < toIndex && index > fromIndex && index <= toIndex) return index - 1;
	if (fromIndex > toIndex && index >= toIndex && index < fromIndex) return index + 1;
	return index;
}

export function resetFileListSession(): void {
	importOrdinalByPath.clear();
	nextImportOrdinal = 0;
	write({ ...INITIAL, selectedFileIndices: new Set<number>() });
}

export function loadFileList(fileList: FileListInfo | null): void {
	importOrdinalByPath.clear();
	nextImportOrdinal = 0;
	if (fileList) recordImportOrder(fileList.files);
	write({
		currentFileList: fileList,
		selectedFileIndex: -1,
		selectedFileIndices: new Set<number>(),
		sortDirection: 'none',
		orderLocked: fileListSession().orderLocked,
	});
}

export function selectFile(
	index: number,
	modifiers: SelectionModifiers = { multi: false, range: false },
): boolean {
	const current = fileListSession();
	const files = current.currentFileList?.files;
	if (!files || index < 0 || index >= files.length) return false;
	if (modifiers.range && current.selectedFileIndex !== -1) {
		const start = Math.min(current.selectedFileIndex, index);
		const end = Math.max(current.selectedFileIndex, index);
		const selectedFileIndices = new Set<number>();
		for (let i = start; i <= end; i += 1) selectedFileIndices.add(i);
		write({ ...current, selectedFileIndex: index, selectedFileIndices });
		return true;
	}
	if (modifiers.multi) {
		const selectedFileIndices = new Set(current.selectedFileIndices);
		if (selectedFileIndices.has(index)) selectedFileIndices.delete(index);
		else selectedFileIndices.add(index);
		write({
			...current,
			...(selectedFileIndices.has(index)
				? { selectedFileIndex: index, selectedFileIndices }
				: ensureAnchor(selectedFileIndices, current.selectedFileIndex)),
		});
		return true;
	}
	write({ ...current, selectedFileIndex: index, selectedFileIndices: new Set([index]) });
	return true;
}

export function selectAll(): boolean {
	const current = fileListSession();
	const count = current.currentFileList?.files.length ?? 0;
	if (count === 0) return false;
	write({
		...current,
		selectedFileIndex: 0,
		selectedFileIndices: new Set(Array.from({ length: count }, (_, i) => i)),
	});
	return true;
}

export function clearSelection(): boolean {
	const current = fileListSession();
	if (current.selectedFileIndices.size === 0 && current.selectedFileIndex === -1) return false;
	write({ ...current, selectedFileIndex: -1, selectedFileIndices: new Set<number>() });
	return true;
}

export function setOrderLocked(locked: boolean): void {
	const current = fileListSession();
	if (current.orderLocked === locked) return;
	write({ ...current, orderLocked: locked });
}

export function reorderFiles(fromIndex: number, toIndex: number): void {
	const current = fileListSession();
	const files = current.currentFileList?.files;
	if (current.orderLocked || !files || fromIndex === toIndex) return;
	if (fromIndex < 0 || toIndex < 0 || fromIndex >= files.length || toIndex >= files.length) return;
	const nextFiles = [...files];
	const [moved] = nextFiles.splice(fromIndex, 1);
	nextFiles.splice(toIndex, 0, moved);
	const selectedFileIndices = new Set(
		[...current.selectedFileIndices].map((index) => mapIndexForMove(index, fromIndex, toIndex)),
	);
	const selectedFileIndex =
		current.selectedFileIndex === -1
			? -1
			: mapIndexForMove(current.selectedFileIndex, fromIndex, toIndex);
	write({
		...replaceFiles(current, nextFiles),
		sortDirection: 'none',
		...ensureAnchor(selectedFileIndices, selectedFileIndex),
	});
}

export function removeFile(index: number): void {
	const current = fileListSession();
	const files = current.currentFileList?.files;
	if (current.orderLocked || !files || index < 0 || index >= files.length) return;
	const removed = files[index];
	if (removed) {
		importOrdinalByPath.delete(removed.path);
		removeFileListCoverThumbnail(removed.path);
	}
	const nextFiles = files.filter((_, fileIndex) => fileIndex !== index);
	const selectedFileIndices = new Set<number>();
	for (const selected of current.selectedFileIndices) {
		if (selected === index) continue;
		selectedFileIndices.add(selected > index ? selected - 1 : selected);
	}
	write({
		...replaceFiles(current, nextFiles),
		...ensureAnchor(
			selectedFileIndices,
			current.selectedFileIndex === index ? -1 : current.selectedFileIndex,
		),
	});
}

export function clearFiles(): void {
	const current = fileListSession();
	if (current.orderLocked || !current.currentFileList) return;
	importOrdinalByPath.clear();
	nextImportOrdinal = 0;
	clearFileListCoverThumbnails();
	write({
		...replaceFiles(current, []),
		selectedFileIndex: -1,
		selectedFileIndices: new Set<number>(),
		sortDirection: 'none',
	});
}

export function toggleSort(): void {
	const current = fileListSession();
	const files = current.currentFileList?.files;
	if (current.orderLocked || !files || files.length <= 1) return;
	const sortDirection: FileListSortDirection =
		current.sortDirection === 'ascending' ? 'descending' : 'ascending';
	const nextFiles = [...files].sort((left, right) => {
		const comparison = pathBasename(left.path, { fallback: 'path' }).localeCompare(
			pathBasename(right.path, { fallback: 'path' }),
			undefined,
			{ numeric: true, sensitivity: 'base' },
		);
		return sortDirection === 'descending' ? -comparison : comparison;
	});
	write({
		...replaceFiles(current, nextFiles),
		sortDirection,
		...remapSelection(files, nextFiles, current.selectedFileIndices, current.selectedFileIndex),
	});
}

export function restoreImportOrder(): void {
	const current = fileListSession();
	const files = current.currentFileList?.files;
	if (current.orderLocked || !files || files.length <= 1) return;
	if (files.some((file) => getImportOrdinal(file.path) === undefined)) return;
	const nextFiles = [...files].sort(
		(left, right) => (getImportOrdinal(left.path) ?? 0) - (getImportOrdinal(right.path) ?? 0),
	);
	write({
		...replaceFiles(current, nextFiles),
		sortDirection: 'none',
		...remapSelection(files, nextFiles, current.selectedFileIndices, current.selectedFileIndex),
	});
}
