export type FileListNavigationCommand =
	| 'previous'
	| 'next'
	| 'first'
	| 'last'
	| 'pagePrevious'
	| 'pageNext';

export const DEFAULT_FILE_LIST_PAGE_STEP = 10;

type KeyboardNavigationEvent = Pick<
	KeyboardEvent,
	'altKey' | 'ctrlKey' | 'key' | 'metaKey' | 'shiftKey'
>;

type NavigationTargetInput = {
	command: FileListNavigationCommand;
	fileCount: number;
	selectedIndex: number;
	pageStep?: number;
};

function hasSelection(index: number, fileCount: number): boolean {
	return index >= 0 && index < fileCount;
}

function clampIndex(index: number, fileCount: number): number {
	return Math.min(Math.max(index, 0), fileCount - 1);
}

export function fileListNavigationCommandFromKey(
	event: KeyboardNavigationEvent,
): FileListNavigationCommand | null {
	if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) {
		return null;
	}

	switch (event.key) {
		case 'ArrowUp':
			return 'previous';
		case 'ArrowDown':
			return 'next';
		case 'Home':
			return 'first';
		case 'End':
			return 'last';
		case 'PageUp':
			return 'pagePrevious';
		case 'PageDown':
			return 'pageNext';
		default:
			return null;
	}
}

export function resolveFileListNavigationTarget({
	command,
	fileCount,
	selectedIndex,
	pageStep = DEFAULT_FILE_LIST_PAGE_STEP,
}: NavigationTargetInput): number | null {
	if (fileCount <= 0) {
		return null;
	}

	const maxIndex = fileCount - 1;
	if (!hasSelection(selectedIndex, fileCount)) {
		return command === 'previous' || command === 'last' ? maxIndex : 0;
	}

	const currentIndex = clampIndex(selectedIndex, fileCount);
	const normalizedPageStep = Math.max(1, Math.floor(pageStep));

	switch (command) {
		case 'previous':
			return Math.max(0, currentIndex - 1);
		case 'next':
			return Math.min(maxIndex, currentIndex + 1);
		case 'first':
			return 0;
		case 'last':
			return maxIndex;
		case 'pagePrevious':
			return Math.max(0, currentIndex - normalizedPageStep);
		case 'pageNext':
			return Math.min(maxIndex, currentIndex + normalizedPageStep);
	}
}

export type FileListKeyCommand =
	| { readonly type: 'navigate'; readonly index: number }
	| { readonly type: 'selectAll' }
	| { readonly type: 'clear' };

export function interpretFileListKeyDown(
	event: KeyboardEvent,
	snapshot: { readonly fileCount: number; readonly selectedAnchor: number },
): FileListKeyCommand | null {
	if (isTextInputTarget(event.target)) {
		return null;
	}
	const command = fileListNavigationCommandFromKey(event);
	if (command) {
		const targetIndex = resolveFileListNavigationTarget({
			command,
			fileCount: snapshot.fileCount,
			selectedIndex: snapshot.selectedAnchor,
		});
		if (targetIndex === null) {
			return null;
		}
		return { type: 'navigate', index: targetIndex };
	}
	const key = event.key.toLowerCase();
	if ((event.metaKey || event.ctrlKey) && key === 'a') {
		return { type: 'selectAll' };
	}
	if (key === 'escape') {
		return { type: 'clear' };
	}
	return null;
}

function isTextInputTarget(target: EventTarget | null): boolean {
	if (!target || !(target instanceof HTMLElement)) {
		return false;
	}
	const tagName = target.tagName.toLowerCase();
	return tagName === 'input' || tagName === 'textarea';
}
