import type { AudioFile, FileListInfo } from '../../types/audio';
import { Atom, AtomRegistry } from '../../lib/effect/appEffect';

type FileListSessionState = {
	currentFileList: FileListInfo | null;
	selectedFileIndex: number;
	selectedFileIndices: Set<number>;
	sortDirection: FileListSortDirection;
	orderLocked: boolean;
};

export type FileListSortDirection = 'none' | 'ascending' | 'descending';

type OrderLockListener = (locked: boolean) => void;

const INITIAL_FILE_LIST_SESSION: FileListSessionState = {
	currentFileList: null,
	selectedFileIndex: -1,
	selectedFileIndices: new Set<number>(),
	sortDirection: 'none',
	orderLocked: false,
};

export const fileListAtomRegistry = AtomRegistry.make();

export const fileListSessionAtom = Atom.make<FileListSessionState>({
	...INITIAL_FILE_LIST_SESSION,
	selectedFileIndices: new Set<number>(),
}).pipe(Atom.keepAlive);

function readSession(): FileListSessionState {
	return fileListAtomRegistry.get(fileListSessionAtom);
}

function writeSession(next: FileListSessionState): void {
	fileListAtomRegistry.set(fileListSessionAtom, next);
}

export function updateFileListSession(
	update: (session: FileListSessionState) => FileListSessionState,
): void {
	fileListAtomRegistry.update(fileListSessionAtom, update);
}

const orderLockListeners = new Set<OrderLockListener>();

// Import arrival order is keyed by path, matching append/dedupe identity. The
// map is deliberately outside reactive state: every list mutation replaces
// the current list and therefore drives derived reads.
const importOrdinalByPath = new Map<string, number>();
let nextImportOrdinal = 0;

export function resetImportOrder(files: AudioFile[]): void {
	importOrdinalByPath.clear();
	nextImportOrdinal = 0;
	for (const file of files) {
		importOrdinalByPath.set(file.path, nextImportOrdinal++);
	}
}

export function recordImportOrder(files: AudioFile[]): void {
	for (const file of files) {
		if (!importOrdinalByPath.has(file.path)) {
			importOrdinalByPath.set(file.path, nextImportOrdinal++);
		}
	}
}

export function removeImportOrdinal(path: string): void {
	importOrdinalByPath.delete(path);
}

export function getImportOrdinal(path: string): number | undefined {
	return importOrdinalByPath.get(path);
}

let fileListRevision = 0;

export type FileListMutationSnapshot = {
	revision: number;
	orderLockExpected: boolean;
};

export function captureFileListMutationSnapshot(): FileListMutationSnapshot {
	return { revision: fileListRevision, orderLockExpected: isOrderLocked() };
}

export function canCommitFileListMutation(snapshot: FileListMutationSnapshot): boolean {
	return (
		fileListRevision === snapshot.revision &&
		readSession().orderLocked === snapshot.orderLockExpected
	);
}

export function fileIdentityKey(file: AudioFile): string {
	return file.inputId ?? file.path;
}

export function findFileIndexByIdentityKey(identityKey: string): number {
	const fileList = getCurrentFileList();
	return fileList ? fileList.files.findIndex((file) => fileIdentityKey(file) === identityKey) : -1;
}

export function replaceFileListFiles(nextFiles: AudioFile[]): void {
	const fileList = getCurrentFileList();
	if (!fileList) return;
	const validCount = nextFiles.filter((file) => file.isValid).length;
	updateFileListSession((session) => ({
		...session,
		currentFileList: {
			...fileList,
			files: nextFiles,
			validCount,
			invalidCount: nextFiles.length - validCount,
		},
	}));
	fileListRevision += 1;
}

export function setCurrentFileList(fileList: FileListInfo | null): void {
	writeSession({
		...readSession(),
		currentFileList: fileList,
		selectedFileIndex: -1,
		selectedFileIndices: new Set<number>(),
		sortDirection: 'none',
	});

	fileListRevision += 1;
}

export function setSelectedIndex(index: number): void {
	updateFileListSession((session) => ({ ...session, selectedFileIndex: index }));
}

export function getCurrentFileList(): FileListInfo | null {
	return readSession().currentFileList;
}

export function getSelectedFileIndex(): number {
	return readSession().selectedFileIndex;
}

export function getSelectedFileIndices(): Set<number> {
	return new Set(readSession().selectedFileIndices);
}

export function getSelectedFiles(): AudioFile[] {
	const fileList = getCurrentFileList();
	if (!fileList) {
		return [];
	}
	return Array.from(getSelectedFileIndices())
		.map((index) => fileList.files[index])
		.filter((file): file is AudioFile => Boolean(file));
}

export function setSelectedFileIndices(indices: Set<number> | number[]): void {
	updateFileListSession((session) => ({
		...session,
		selectedFileIndices: new Set(indices),
	}));
}

export function addToSelectedIndices(index: number): void {
	updateFileListSession((session) => {
		const selectedFileIndices = new Set(session.selectedFileIndices);
		selectedFileIndices.add(index);
		return { ...session, selectedFileIndices };
	});
}

export function removeFromSelectedIndices(index: number): void {
	updateFileListSession((session) => {
		const selectedFileIndices = new Set(session.selectedFileIndices);
		selectedFileIndices.delete(index);
		return { ...session, selectedFileIndices };
	});
}

export function clearSelectedIndices(): void {
	updateFileListSession((session) => ({
		...session,
		selectedFileIndices: new Set<number>(),
	}));
}

export function getSortDirection(): FileListSortDirection {
	return readSession().sortDirection;
}

export function setSortDirection(direction: FileListSortDirection): void {
	updateFileListSession((session) => ({ ...session, sortDirection: direction }));
}

/** Legacy test/helper aliases; UI state uses the explicit direction. */
export function getSortAscending(): boolean {
	return readSession().sortDirection !== 'descending';
}

export function setSortAscending(ascending: boolean): void {
	updateFileListSession((session) => ({
		...session,
		sortDirection: ascending ? 'ascending' : 'descending',
	}));
}

export function setOrderLocked(locked: boolean): void {
	if (readSession().orderLocked === locked) {
		return;
	}

	updateFileListSession((session) => ({ ...session, orderLocked: locked }));
	for (const listener of [...orderLockListeners]) {
		listener(locked);
	}
}

export function isOrderLocked(): boolean {
	return readSession().orderLocked;
}

export function onOrderLockChange(listener: OrderLockListener): () => void {
	orderLockListeners.add(listener);
	return () => {
		orderLockListeners.delete(listener);
	};
}
