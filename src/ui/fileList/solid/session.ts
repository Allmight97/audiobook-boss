import { pathBasename } from '../../../lib/path/basename';
import type { AudioFile, FileListInfo } from '../../../types/audio';
import { Atom, AtomRegistry } from '../../../lib/effect/appEffect';

export type FileListSortDirection = 'none' | 'ascending' | 'descending';

export type FileListSession = {
	currentFileList: FileListInfo | null;
	selectedFileIndex: number;
	selectedFileIndices: ReadonlySet<number>;
	sortDirection: FileListSortDirection;
	orderLocked: boolean;
};

export type SelectionModifiers = { multi: boolean; range: boolean };

const INITIAL_SESSION: FileListSession = {
	currentFileList: null,
	selectedFileIndex: -1,
	selectedFileIndices: new Set<number>(),
	sortDirection: 'none',
	orderLocked: false,
};

export const fileListRegistry = AtomRegistry.make();

export const fileListSessionAtom = Atom.make<FileListSession>({
	...INITIAL_SESSION,
	selectedFileIndices: new Set<number>(),
}).pipe(Atom.keepAlive);

const importOrdinalByPath = new Map<string, number>();
let nextImportOrdinal = 0;

function session(): FileListSession {
	return fileListRegistry.get(fileListSessionAtom);
}

function write(next: FileListSession): void {
	fileListRegistry.set(fileListSessionAtom, next);
}

function update(patch: (current: FileListSession) => FileListSession): void {
	fileListRegistry.update(fileListSessionAtom, patch);
}

function fileIdentityKey(file: AudioFile): string {
	return file.inputId ?? file.path;
}

function recordImportOrder(files: readonly AudioFile[]): void {
	for (const file of files) {
		if (!importOrdinalByPath.has(file.path)) {
			importOrdinalByPath.set(file.path, nextImportOrdinal++);
		}
	}
}

export function getImportOrdinal(path: string): number | undefined {
	return importOrdinalByPath.get(path);
}

function replaceFiles(sessionState: FileListSession, nextFiles: AudioFile[]): FileListSession {
	const fileList = sessionState.currentFileList;
	if (!fileList) return sessionState;
	const validCount = nextFiles.filter((file) => file.isValid).length;
	return {
		...sessionState,
		currentFileList: {
			...fileList,
			files: nextFiles,
			validCount,
			invalidCount: nextFiles.length - validCount,
		},
	};
}

function reindexAfterRemoval(selected: ReadonlySet<number>, removedIndex: number): Set<number> {
	const next = new Set<number>();
	for (const index of selected) {
		if (index === removedIndex) continue;
		next.add(index > removedIndex ? index - 1 : index);
	}
	return next;
}

function mapIndexForMove(index: number, fromIndex: number, toIndex: number): number {
	if (index === fromIndex) return toIndex;
	if (fromIndex < toIndex && index > fromIndex && index <= toIndex) return index - 1;
	if (fromIndex > toIndex && index >= toIndex && index < fromIndex) return index + 1;
	return index;
}

function ensureAnchor(
	selected: ReadonlySet<number>,
	anchor: number,
): { selectedFileIndex: number; selectedFileIndices: ReadonlySet<number> } {
	if (selected.size === 0) return { selectedFileIndex: -1, selectedFileIndices: selected };
	if (anchor >= 0 && selected.has(anchor)) {
		return { selectedFileIndex: anchor, selectedFileIndices: selected };
	}
	const sorted = [...selected].sort((a, b) => a - b);
	const last = sorted[sorted.length - 1] ?? -1;
	return { selectedFileIndex: last, selectedFileIndices: selected };
}

export function resetFileList(): void {
	importOrdinalByPath.clear();
	nextImportOrdinal = 0;
	write({ ...INITIAL_SESSION, selectedFileIndices: new Set<number>() });
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
		orderLocked: session().orderLocked,
	});
}

export function selectFile(
	index: number,
	modifiers: SelectionModifiers = { multi: false, range: false },
): boolean {
	const current = session();
	const files = current.currentFileList?.files;
	if (!files || index < 0 || index >= files.length) return false;
	const { multi, range } = modifiers;
	if (range && current.selectedFileIndex !== -1) {
		const start = Math.min(current.selectedFileIndex, index);
		const end = Math.max(current.selectedFileIndex, index);
		const selectedFileIndices = new Set<number>();
		for (let i = start; i <= end; i += 1) selectedFileIndices.add(i);
		write({ ...current, selectedFileIndex: index, selectedFileIndices });
		return true;
	}
	if (multi) {
		const selectedFileIndices = new Set(current.selectedFileIndices);
		if (selectedFileIndices.has(index)) selectedFileIndices.delete(index);
		else {
			selectedFileIndices.add(index);
			write({ ...current, selectedFileIndex: index, selectedFileIndices });
			return true;
		}
		write({ ...current, ...ensureAnchor(selectedFileIndices, current.selectedFileIndex) });
		return true;
	}
	write({
		...current,
		selectedFileIndex: index,
		selectedFileIndices: new Set([index]),
	});
	return true;
}

export function selectAll(): boolean {
	const current = session();
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
	const current = session();
	if (current.selectedFileIndices.size === 0 && current.selectedFileIndex === -1) return false;
	write({ ...current, selectedFileIndex: -1, selectedFileIndices: new Set<number>() });
	return true;
}

export function setOrderLocked(locked: boolean): void {
	if (session().orderLocked === locked) return;
	update((current) => ({ ...current, orderLocked: locked }));
}

export function reorderFiles(fromIndex: number, toIndex: number): void {
	const current = session();
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

export function moveFile(index: number, delta: -1 | 1): void {
	reorderFiles(index, index + delta);
}

export function removeFile(index: number): void {
	const current = session();
	const files = current.currentFileList?.files;
	if (current.orderLocked || !files || index < 0 || index >= files.length) return;
	const removed = files[index];
	if (removed) importOrdinalByPath.delete(removed.path);
	const nextFiles = files.filter((_, fileIndex) => fileIndex !== index);
	const selectedFileIndices = reindexAfterRemoval(current.selectedFileIndices, index);
	write({
		...replaceFiles(current, nextFiles),
		...ensureAnchor(
			selectedFileIndices,
			current.selectedFileIndex === index ? -1 : current.selectedFileIndex,
		),
	});
}

export function clearFiles(): void {
	const current = session();
	if (current.orderLocked || !current.currentFileList) return;
	importOrdinalByPath.clear();
	nextImportOrdinal = 0;
	write({
		...replaceFiles(current, []),
		selectedFileIndex: -1,
		selectedFileIndices: new Set<number>(),
		sortDirection: 'none',
	});
}

export function toggleSort(): void {
	const current = session();
	const files = current.currentFileList?.files;
	if (current.orderLocked || !files || files.length <= 1) return;
	const sortDirection: FileListSortDirection =
		current.sortDirection === 'ascending' ? 'descending' : 'ascending';
	const selectedKeys = new Set(
		[...current.selectedFileIndices]
			.map((index) => files[index])
			.filter((file): file is AudioFile => Boolean(file))
			.map(fileIdentityKey),
	);
	const anchorKey = files[current.selectedFileIndex]
		? fileIdentityKey(files[current.selectedFileIndex])
		: null;
	const nextFiles = [...files].sort((left, right) => {
		const comparison = pathBasename(left.path, { fallback: 'path' }).localeCompare(
			pathBasename(right.path, { fallback: 'path' }),
			undefined,
			{ numeric: true, sensitivity: 'base' },
		);
		return sortDirection === 'descending' ? -comparison : comparison;
	});
	const selectedFileIndices = new Set(
		nextFiles.flatMap((file, index) => (selectedKeys.has(fileIdentityKey(file)) ? [index] : [])),
	);
	const selectedFileIndex = anchorKey
		? nextFiles.findIndex((file) => fileIdentityKey(file) === anchorKey)
		: -1;
	write({
		...replaceFiles(current, nextFiles),
		sortDirection,
		selectedFileIndex,
		selectedFileIndices,
	});
}

export function restoreImportOrder(): void {
	const current = session();
	const files = current.currentFileList?.files;
	if (current.orderLocked || !files || files.length <= 1) return;
	if (files.some((file) => getImportOrdinal(file.path) === undefined)) return;
	const selectedKeys = new Set(
		[...current.selectedFileIndices]
			.map((index) => files[index])
			.filter((file): file is AudioFile => Boolean(file))
			.map(fileIdentityKey),
	);
	const anchorKey = files[current.selectedFileIndex]
		? fileIdentityKey(files[current.selectedFileIndex])
		: null;
	const nextFiles = [...files].sort(
		(left, right) => (getImportOrdinal(left.path) ?? 0) - (getImportOrdinal(right.path) ?? 0),
	);
	const selectedFileIndices = new Set(
		nextFiles.flatMap((file, index) => (selectedKeys.has(fileIdentityKey(file)) ? [index] : [])),
	);
	const selectedFileIndex = anchorKey
		? nextFiles.findIndex((file) => fileIdentityKey(file) === anchorKey)
		: -1;
	write({
		...replaceFiles(current, nextFiles),
		sortDirection: 'none',
		selectedFileIndex,
		selectedFileIndices,
	});
}
