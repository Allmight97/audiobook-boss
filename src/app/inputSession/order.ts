import type { AudioFile } from '../../types/audio';
import { pathBasename } from '../../lib/path/basename';
import { buildFileListInfoFromFiles, buildSelectedDecoderByPath } from './appendResult';
import {
	reindexSelectionAfterMove,
	reindexSelectionAfterRemoval,
	swapSelectionIndices,
} from './selection';
import { emptyInputSession, fileIdentityKey, type InputSessionState } from './types';

export function replaceFileListFiles(
	session: InputSessionState,
	files: ReadonlyArray<AudioFile>,
): InputSessionState {
	const decoderByPath = session.fileList ? buildSelectedDecoderByPath(session.fileList) : new Map();
	return {
		...session,
		fileList: buildFileListInfoFromFiles([...files], decoderByPath),
	};
}

export function removeFileFromSession(
	session: InputSessionState,
	index: number,
): { readonly session: InputSessionState; readonly removed: AudioFile | null } {
	if (session.orderLocked) {
		return { session, removed: null };
	}
	const files = session.fileList?.files ?? [];
	if (index < 0 || index >= files.length) {
		return { session, removed: null };
	}
	const removed = files[index] ?? null;
	if (!removed) {
		return { session, removed: null };
	}
	const nextFiles = files.filter((_, fileIndex) => fileIndex !== index);
	const importOrdinalByPath = { ...session.importOrdinalByPath };
	delete importOrdinalByPath[removed.path];
	const next = reindexSelectionAfterRemoval(replaceFileListFiles(session, nextFiles), index);
	return {
		session: {
			...next,
			importOrdinalByPath,
		},
		removed,
	};
}

export function clearAllFilesFromSession(session: InputSessionState): InputSessionState {
	if (session.orderLocked || !session.fileList) {
		return session;
	}
	return {
		...emptyInputSession(),
		supportText: session.supportText,
		orderLocked: session.orderLocked,
	};
}

export function moveFileInSession(
	session: InputSessionState,
	index: number,
	direction: 'up' | 'down',
): InputSessionState {
	if (session.orderLocked) {
		return session;
	}
	const files = session.fileList?.files ?? [];
	const target = direction === 'up' ? index - 1 : index + 1;
	if (index < 0 || index >= files.length || target < 0 || target >= files.length) {
		return session;
	}
	const nextFiles = [...files];
	const current = nextFiles[index];
	const swapped = nextFiles[target];
	if (!current || !swapped) {
		return session;
	}
	nextFiles[index] = swapped;
	nextFiles[target] = current;
	return {
		...swapSelectionIndices(replaceFileListFiles(session, nextFiles), index, target),
		sortDirection: 'none',
	};
}

export function reorderFilesInSession(
	session: InputSessionState,
	fromIndex: number,
	toIndex: number,
): InputSessionState {
	if (session.orderLocked) {
		return session;
	}
	const files = session.fileList?.files ?? [];
	if (
		fromIndex === toIndex ||
		fromIndex < 0 ||
		toIndex < 0 ||
		fromIndex >= files.length ||
		toIndex >= files.length
	) {
		return session;
	}
	const nextFiles = [...files];
	const [moved] = nextFiles.splice(fromIndex, 1);
	if (!moved) {
		return session;
	}
	nextFiles.splice(toIndex, 0, moved);
	return {
		...reindexSelectionAfterMove(replaceFileListFiles(session, nextFiles), fromIndex, toIndex),
		sortDirection: 'none',
	};
}

export function sortFilesInSession(session: InputSessionState): InputSessionState {
	if (session.orderLocked) {
		return session;
	}
	const files = session.fileList?.files ?? [];
	if (files.length <= 1) {
		return session;
	}
	const selectedKeys = selectedIdentityKeys(session);
	const selectedAnchorKey =
		session.selectedAnchor >= 0 ? identityKeyAt(session, session.selectedAnchor) : undefined;
	const nextSortDirection = session.sortDirection === 'ascending' ? 'descending' : 'ascending';
	const nextFiles = [...files].sort((left, right) => {
		const comparison = pathBasename(left.path, { fallback: 'path' }).localeCompare(
			pathBasename(right.path, { fallback: 'path' }),
			undefined,
			{ numeric: true, sensitivity: 'base' },
		);
		return nextSortDirection === 'ascending' ? comparison : -comparison;
	});
	return withPreservedSelection(
		{ ...replaceFileListFiles(session, nextFiles), sortDirection: nextSortDirection },
		selectedKeys,
		selectedAnchorKey,
	);
}

export function restoreImportOrderInSession(session: InputSessionState): InputSessionState {
	if (session.orderLocked) {
		return session;
	}
	const files = session.fileList?.files ?? [];
	if (files.length <= 1) {
		return session;
	}
	if (files.some((file) => session.importOrdinalByPath[file.path] === undefined)) {
		return session;
	}
	const selectedKeys = selectedIdentityKeys(session);
	const selectedAnchorKey =
		session.selectedAnchor >= 0 ? identityKeyAt(session, session.selectedAnchor) : undefined;
	const nextFiles = [...files].sort(
		(left, right) =>
			(session.importOrdinalByPath[left.path] ?? 0) -
			(session.importOrdinalByPath[right.path] ?? 0),
	);
	return withPreservedSelection(
		{ ...replaceFileListFiles(session, nextFiles), sortDirection: 'none' },
		selectedKeys,
		selectedAnchorKey,
	);
}

function selectedIdentityKeys(session: InputSessionState): ReadonlySet<string> {
	const files = session.fileList?.files ?? [];
	return new Set(
		session.selectedIndices
			.map((index) => files[index])
			.filter((file): file is AudioFile => Boolean(file))
			.map((file) => fileIdentityKey(file)),
	);
}

function identityKeyAt(session: InputSessionState, index: number): string | undefined {
	const file = session.fileList?.files[index];
	return file ? fileIdentityKey(file) : undefined;
}

function withPreservedSelection(
	session: InputSessionState,
	selectedKeys: ReadonlySet<string>,
	selectedAnchorKey: string | undefined,
): InputSessionState {
	const files = session.fileList?.files ?? [];
	const selectedIndices = files.flatMap((file, index) =>
		selectedKeys.has(fileIdentityKey(file)) ? [index] : [],
	);
	const selectedAnchor = selectedAnchorKey
		? files.findIndex((file) => fileIdentityKey(file) === selectedAnchorKey)
		: (selectedIndices[selectedIndices.length - 1] ?? -1);
	return {
		...session,
		selectedIndices,
		selectedAnchor: selectedAnchor >= 0 ? selectedAnchor : (selectedIndices[0] ?? -1),
	};
}

export function setOrderLockedInSession(
	session: InputSessionState,
	orderLocked: boolean,
): InputSessionState {
	if (session.orderLocked === orderLocked) {
		return session;
	}
	return { ...session, orderLocked };
}

export function inputIdsInSession(session: InputSessionState): ReadonlyArray<string> {
	return (session.fileList?.files ?? [])
		.map((file) => file.inputId)
		.filter((inputId): inputId is string => Boolean(inputId));
}
