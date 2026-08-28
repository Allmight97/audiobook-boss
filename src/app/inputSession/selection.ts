import type { InputSessionState, SelectionModifiers } from './types';

export function selectFileInSession(
	session: InputSessionState,
	index: number,
	modifiers: SelectionModifiers,
): InputSessionState {
	const files = session.fileList?.files ?? [];
	if (index < 0 || index >= files.length) {
		return session;
	}

	if (modifiers.range && session.selectedAnchor !== -1) {
		const start = Math.min(session.selectedAnchor, index);
		const end = Math.max(session.selectedAnchor, index);
		const selectedIndices = Array.from({ length: end - start + 1 }, (_, offset) => start + offset);
		return { ...session, selectedIndices, selectedAnchor: index };
	}

	if (modifiers.multi) {
		const selected = new Set(session.selectedIndices);
		if (selected.has(index)) {
			selected.delete(index);
		} else {
			selected.add(index);
		}
		const selectedIndices = Array.from(selected).sort((left, right) => left - right);
		const selectedAnchor = selected.has(index)
			? index
			: (selectedIndices[selectedIndices.length - 1] ?? -1);
		return { ...session, selectedIndices, selectedAnchor };
	}

	return { ...session, selectedIndices: [index], selectedAnchor: index };
}

export function selectAllInSession(session: InputSessionState): InputSessionState {
	const count = session.fileList?.files.length ?? 0;
	if (count === 0) {
		return session;
	}
	return {
		...session,
		selectedIndices: Array.from({ length: count }, (_, index) => index),
		selectedAnchor: 0,
	};
}

export function clearSelectionInSession(session: InputSessionState): InputSessionState {
	if (session.selectedIndices.length === 0 && session.selectedAnchor === -1) {
		return session;
	}
	return { ...session, selectedIndices: [], selectedAnchor: -1 };
}

export function reindexSelectionAfterRemoval(
	session: InputSessionState,
	removedIndex: number,
): InputSessionState {
	const selectedIndices = session.selectedIndices
		.filter((index) => index !== removedIndex)
		.map((index) => (index > removedIndex ? index - 1 : index))
		.sort((left, right) => left - right);
	return withAnchoredSelection(session, selectedIndices);
}

export function reindexSelectionAfterMove(
	session: InputSessionState,
	fromIndex: number,
	toIndex: number,
): InputSessionState {
	const selectedIndices = session.selectedIndices
		.map((index) => mapIndexForMove(index, fromIndex, toIndex))
		.sort((left, right) => left - right);
	const selectedAnchor =
		session.selectedAnchor === -1
			? -1
			: mapIndexForMove(session.selectedAnchor, fromIndex, toIndex);
	return { ...session, selectedIndices, selectedAnchor };
}

export function swapSelectionIndices(
	session: InputSessionState,
	indexA: number,
	indexB: number,
): InputSessionState {
	const selectedIndices = session.selectedIndices
		.map((index) => {
			if (index === indexA) return indexB;
			if (index === indexB) return indexA;
			return index;
		})
		.sort((left, right) => left - right);
	let selectedAnchor = session.selectedAnchor;
	if (selectedAnchor === indexA) {
		selectedAnchor = indexB;
	} else if (selectedAnchor === indexB) {
		selectedAnchor = indexA;
	}
	return withAnchoredSelection({ ...session, selectedAnchor }, selectedIndices);
}

export function mapIndexForMove(index: number, fromIndex: number, toIndex: number): number {
	if (index === fromIndex) return toIndex;
	if (fromIndex < toIndex && index > fromIndex && index <= toIndex) return index - 1;
	if (fromIndex > toIndex && index >= toIndex && index < fromIndex) return index + 1;
	return index;
}

function withAnchoredSelection(
	session: InputSessionState,
	selectedIndices: ReadonlyArray<number>,
): InputSessionState {
	const selected = new Set(selectedIndices);
	let selectedAnchor = session.selectedAnchor;
	if (selected.size === 0) {
		selectedAnchor = -1;
	} else if (selectedAnchor < 0 || !selected.has(selectedAnchor)) {
		selectedAnchor = selectedIndices[selectedIndices.length - 1] ?? -1;
	}
	return { ...session, selectedIndices, selectedAnchor };
}
