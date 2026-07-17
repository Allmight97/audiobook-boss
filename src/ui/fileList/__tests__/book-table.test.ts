import { fireEvent, render, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { tauriClient } from '../../../lib/tauri/client';
import type { AudioFile, FileListInfo } from '../../../types/audio';
import type { OperationSnapshot } from '../../../types/workRuntime';
import { readWorkActivityByInputId } from '../../workCenter';
import { applyOperationSnapshot } from '../../workCenter/state.svelte';
import FileListIsland from '../FileListIsland.svelte';
import {
	getCurrentFileList,
	getSelectedFileIndex,
	getSelectedFileIndices,
	setCurrentFileList,
	setOrderLocked,
	setSelectedFileIndices,
	setSelectedIndex,
} from '../state.svelte';
import {
	cacheMetadataForFile,
	clearMetadataSession,
	saveMetadataFromUI,
	stageMetadataIntentPatch,
} from '../../metadataSession';
import { setMetadataSurfacePresentation } from '../metadataPanel';

function file(path: string, inputId: string, isValid = true): AudioFile {
	return {
		path,
		inputId,
		isValid,
		duration: 60,
		size: 1024,
		format: 'm4b',
		error: isValid ? undefined : 'Unreadable audio',
	};
}

function stubElementFromPoint(hit: () => Element | null): () => void {
	const original = document.elementFromPoint;
	document.elementFromPoint = hit as typeof document.elementFromPoint;
	return () => {
		document.elementFromPoint = original;
	};
}

function rowFor(
	screen: { getByRole: (role: string, options: { name: string }) => HTMLElement },
	name: string,
): HTMLTableRowElement {
	const row = screen.getByRole('button', { name: `Edit metadata for ${name}` }).closest('tr');
	if (!row) throw new Error(`Expected a file-list row for ${name}`);
	return row as HTMLTableRowElement;
}

function fileList(...files: AudioFile[]): FileListInfo {
	return {
		files,
		selectedDecoders: files.map(() => null),
		totalDuration: files.length * 60,
		totalSize: files.length * 1024,
		validCount: files.filter((item) => item.isValid).length,
		invalidCount: files.filter((item) => !item.isValid).length,
	};
}

function batchSnapshot(inputId: string): OperationSnapshot {
	return {
		operationId: `operation-${inputId}`,
		sequence: Date.now(),
		kind: 'processingBatch',
		status: 'running',
		title: 'Processing',
		createdAtMs: 1,
		startedAtMs: 1,
		finishedAtMs: undefined,
		cancellable: true,
		cancelRequested: false,
		lanes: ['analysis'],
		sourceInputIds: [],
		progress: {
			stage: 'converting',
			percentage: 50,
			message: 'Converting',
			currentItemIndex: undefined,
			totalItems: 1,
			bytesDownloaded: undefined,
			bytesTotal: undefined,
			etaSeconds: undefined,
		},
		children: [
			{
				childJobId: `child-${inputId}`,
				operationId: `operation-${inputId}`,
				label: 'Book',
				status: 'running',
				lane: 'analysis',
				progress: {
					stage: 'converting',
					percentage: 50,
					message: 'Converting',
					currentItemIndex: undefined,
					totalItems: 1,
					bytesDownloaded: undefined,
					bytesTotal: undefined,
					etaSeconds: undefined,
				},
				sourcePath: undefined,
				inputIndex: 0,
				inputId,
				jobId: undefined,
				cancellable: true,
				cancelRequested: false,
				message: undefined,
			},
		],
		terminalSummary: undefined,
		warnings: [],
		errors: [],
		logTail: [],
	};
}

describe('v3 book table', () => {
	beforeEach(() => {
		clearMetadataSession();
		setOrderLocked(false);
		setCurrentFileList(
			fileList(file('/books/ready.m4b', 'ready'), file('/books/bad.m4b', 'bad', false)),
		);
		setSelectedFileIndices([]);
		setSelectedIndex(-1);
	});

	afterEach(() => {
		vi.restoreAllMocks();
		document.documentElement.removeAttribute('data-density');
		setMetadataSurfacePresentation(null);
	});

	it('keeps the filename primary and adds the title as a secondary line once metadata caches', async () => {
		const screen = render(FileListIsland, { props: { readWorkActivityByInputId } });

		// First paint precedes the async backend metadata read: filename only.
		expect(screen.getByText('ready.m4b')).toBeInTheDocument();
		expect(screen.queryByText(/The Way of Kings/)).toBeNull();

		cacheMetadataForFile('/books/ready.m4b', {
			title: 'The Way of Kings',
			artist: 'Brandon Sanderson',
		});
		await tick();

		// Filename never disappears — it is the queue's row identity.
		expect(screen.getByText('ready.m4b')).toBeInTheDocument();
		expect(screen.getByText(/The Way of Kings/)).toBeInTheDocument();
	});

	it('shows analyzed tag titles beside filenames with zero selection', () => {
		const tagged = {
			...file('/books/tagged.m4b', 'tagged'),
			tagTitle: 'Analyzed Title',
			tagArtist: 'Analyzed Artist',
		};
		setCurrentFileList(fileList(tagged));
		const screen = render(FileListIsland);

		expect(getSelectedFileIndex()).toBe(-1);
		expect(screen.getByText('tagged.m4b')).toBeInTheDocument();
		expect(screen.getByText(/Analyzed Title/)).toBeInTheDocument();
	});

	it('does not resurrect analyzed row tags after a staged clear succeeds', async () => {
		const saveMetadata = vi.spyOn(tauriClient, 'saveMetadataBatch').mockResolvedValue({
			summary: { total: 1, succeeded: 1, skipped: 0, cancelled: 0, failed: 0 },
			results: [
				{
					inputIndex: 0,
					filePath: '/books/tagged.m4b',
					status: 'success',
					message: 'saved',
				},
			],
		});
		const tagged = {
			...file('/books/tagged.m4b', 'tagged'),
			tagTitle: 'Analyzed Title',
			tagArtist: 'Analyzed Artist',
		};
		setCurrentFileList(fileList(tagged));
		cacheMetadataForFile(tagged.path, { title: 'Session title', artist: 'Session artist' });
		expect(
			stageMetadataIntentPatch(tagged.path, {
				title: { op: 'clear' },
				artist: { op: 'clear' },
			}),
		).toBe('staged');
		const screen = render(FileListIsland);

		expect(screen.getByText('tagged.m4b')).toBeInTheDocument();
		expect(screen.queryByText(/Analyzed Title/)).toBeNull();

		await saveMetadataFromUI();
		await waitFor(() => expect(saveMetadata).toHaveBeenCalledTimes(1));
		await tick();

		expect(screen.getByText('tagged.m4b')).toBeInTheDocument();
		expect(screen.queryByText(/Analyzed Title/)).toBeNull();
	});

	it('loads row thumbnails independently and keeps a saved cover clear authoritative', async () => {
		const readThumbnail = vi
			.spyOn(tauriClient, 'readAudioCoverThumbnail')
			.mockImplementation(async (path) => (path.endsWith('ready.m4b') ? [1, 2, 3] : null));
		const saveMetadata = vi.spyOn(tauriClient, 'saveMetadataBatch').mockResolvedValue({
			summary: { total: 1, succeeded: 1, skipped: 0, cancelled: 0, failed: 0 },
			results: [
				{
					inputIndex: 0,
					filePath: '/books/ready.m4b',
					status: 'success',
					message: 'saved',
				},
			],
		});
		const screen = render(FileListIsland);
		const readyRow = rowFor(screen, 'ready.m4b');

		await waitFor(() => expect(readyRow.querySelector('img')).toBeInTheDocument());
		expect(getSelectedFileIndex()).toBe(-1);
		expect(readThumbnail).toHaveBeenCalledWith('/books/ready.m4b');
		expect(readThumbnail).not.toHaveBeenCalledWith('/books/bad.m4b');

		cacheMetadataForFile('/books/ready.m4b', { cover_art: [9] });
		expect(stageMetadataIntentPatch('/books/ready.m4b', { cover_art: { op: 'clear' } })).toBe(
			'staged',
		);
		await tick();

		expect(readyRow.querySelector('img')).not.toBeInTheDocument();

		await saveMetadataFromUI();
		await waitFor(() => expect(saveMetadata).toHaveBeenCalledTimes(1));
		await tick();

		expect(readyRow.querySelector('img')).not.toBeInTheDocument();
	});

	it('renders derived activity badges but makes invalid input an error', async () => {
		applyOperationSnapshot(batchSnapshot('ready'));
		const screen = render(FileListIsland, { props: { readWorkActivityByInputId } });

		await waitFor(() => {
			expect(screen.getByText('running')).toBeInTheDocument();
		});
		expect(screen.getByText('error')).toHaveAttribute('title', 'Unreadable audio');
	});

	it('renders skipped work as skipped rather than done', async () => {
		const snapshot = batchSnapshot('ready');
		snapshot.children[0] = { ...snapshot.children[0]!, status: 'skipped' };
		applyOperationSnapshot(snapshot);
		const screen = render(FileListIsland, { props: { readWorkActivityByInputId } });

		await waitFor(() => expect(screen.getByText('skipped')).toBeInTheDocument());
	});

	it('uses a group for keyboard navigation and exposes selection state on rows', async () => {
		const screen = render(FileListIsland);
		setSelectedFileIndices([0]);
		setSelectedIndex(0);
		await tick();

		expect(screen.getByRole('group', { name: 'Audio files' })).not.toHaveAttribute(
			'aria-multiselectable',
		);
		expect(rowFor(screen, 'ready.m4b')).toHaveAttribute('aria-selected', 'true');
		expect(rowFor(screen, 'bad.m4b')).toHaveAttribute('aria-selected', 'false');
	});

	it('exposes reorder shortcuts and changes the row grip when ordering locks', async () => {
		const screen = render(FileListIsland);
		const keyboardGroup = screen.getByRole('group', { name: 'Audio files' });
		const table = screen.getByTestId('book-table');
		const row = rowFor(screen, 'ready.m4b');
		const grip = row.querySelector('.file-list-reorder-grip');

		expect(keyboardGroup).toHaveAttribute('aria-keyshortcuts', 'Alt+ArrowUp Alt+ArrowDown');
		expect(getComputedStyle(table).tableLayout).toBe('fixed');
		expect(getComputedStyle(table).userSelect).toBe('none');
		expect(grip).toHaveAttribute(
			'title',
			'Drag to reorder. Move the selected file with Alt+ArrowUp or Alt+ArrowDown.',
		);
		// Reorder is pointer-driven: nothing in the table may enter the OS drag layer.
		expect(row).toHaveAttribute('draggable', 'false');
		expect(grip).not.toHaveAttribute('draggable');

		setOrderLocked(true);
		await tick();

		expect(row).toHaveClass('order-locked');
		expect(row).toHaveAttribute('draggable', 'false');
		expect(grip).toHaveTextContent('⠿');
		expect(grip).toHaveAttribute('title', 'File order is locked');
	});

	it('keeps text non-draggable while retaining full book-title tooltips', async () => {
		const screen = render(FileListIsland);
		const firstTitle = screen.getByRole('button', { name: 'Edit metadata for ready.m4b' });
		const secondRow = rowFor(screen, 'bad.m4b');
		const restoreHitTest = stubElementFromPoint(() => secondRow);

		try {
			expect(firstTitle).toHaveAttribute('title', 'ready.m4b');
			// Only the grip starts a reorder: a pointer drag from the title text
			// must never move rows, even past the movement threshold.
			await fireEvent.pointerDown(firstTitle, { pointerId: 1, button: 0, clientX: 0, clientY: 0 });
			await fireEvent.pointerMove(window, { pointerId: 1, clientX: 0, clientY: 40 });
			await fireEvent.pointerUp(window, { pointerId: 1 });

			expect(getCurrentFileList()?.files.map((item) => item.path)).toEqual([
				'/books/ready.m4b',
				'/books/bad.m4b',
			]);
		} finally {
			restoreHitTest();
		}
	});

	it('hides comfortable-only columns at compact density', async () => {
		const screen = render(FileListIsland);
		document.documentElement.dataset.density = 'compact';
		await tick();

		expect(screen.getByText('Size')).toHaveClass('file-list-comfortable-only');
	});

	it('selects all rows from the keyboard with Cmd/Ctrl+A', async () => {
		const screen = render(FileListIsland);
		await fireEvent.keyDown(screen.getByRole('group', { name: 'Audio files' }), {
			key: 'a',
			metaKey: true,
		});

		await waitFor(() => {
			expect(rowFor(screen, 'ready.m4b')).toHaveAttribute('aria-selected', 'true');
			expect(rowFor(screen, 'bad.m4b')).toHaveAttribute('aria-selected', 'true');
		});
	});

	it('shift-clicking a row title selects the active-to-clicked range', async () => {
		setCurrentFileList(
			fileList(
				file('/books/one.m4b', 'one'),
				file('/books/two.m4b', 'two'),
				file('/books/three.m4b', 'three'),
			),
		);
		setSelectedFileIndices([0]);
		setSelectedIndex(0);
		const screen = render(FileListIsland);

		await fireEvent.click(screen.getByRole('button', { name: 'Edit metadata for three.m4b' }), {
			shiftKey: true,
		});

		await waitFor(() => {
			expect(rowFor(screen, 'one.m4b')).toHaveAttribute('aria-selected', 'true');
			expect(rowFor(screen, 'two.m4b')).toHaveAttribute('aria-selected', 'true');
			expect(rowFor(screen, 'three.m4b')).toHaveAttribute('aria-selected', 'true');
		});
	});

	it('sorts by numeric filename order and ignores metadata titles', async () => {
		const tenth = file('/books/10 - Last Chapter.mp3', 'tenth');
		const second = file('/books/2 - Early Chapter.mp3', 'second');
		setCurrentFileList(fileList(tenth, second));
		// Titles ordered opposite to filenames: sorting must not consult them.
		cacheMetadataForFile(tenth.path, { title: 'AAA title' });
		cacheMetadataForFile(second.path, { title: 'ZZZ title' });
		const screen = render(FileListIsland);
		const header = screen.getByRole('button', { name: 'File' });

		expect(header.closest('th')).toHaveAttribute('aria-sort', 'none');
		await fireEvent.click(header);
		await waitFor(() => {
			expect(getCurrentFileList()?.files.map((item) => item.path)).toEqual([
				'/books/2 - Early Chapter.mp3',
				'/books/10 - Last Chapter.mp3',
			]);
		});
		expect(header.closest('th')).toHaveAttribute('aria-sort', 'ascending');

		await fireEvent.click(header);
		await waitFor(() => {
			expect(getCurrentFileList()?.files.map((item) => item.path)).toEqual([
				'/books/10 - Last Chapter.mp3',
				'/books/2 - Early Chapter.mp3',
			]);
		});
		expect(header.closest('th')).toHaveAttribute('aria-sort', 'descending');
	});

	it('preserves selected file paths and the active file when sorting', async () => {
		const zeta = file('/books/zeta.m4b', 'zeta');
		const alpha = file('/books/alpha.m4b', 'alpha');
		const beta = file('/books/beta.m4b', 'beta');
		setCurrentFileList(fileList(zeta, alpha, beta));
		setSelectedFileIndices([0, 2]);
		setSelectedIndex(2);
		const screen = render(FileListIsland);

		await fireEvent.click(screen.getByRole('button', { name: 'File' }));
		await waitFor(() => {
			expect(getCurrentFileList()?.files.map((item) => item.path)).toEqual([
				'/books/alpha.m4b',
				'/books/beta.m4b',
				'/books/zeta.m4b',
			]);
		});

		expect(rowFor(screen, 'beta.m4b')).toHaveAttribute('aria-selected', 'true');
		expect(rowFor(screen, 'zeta.m4b')).toHaveAttribute('aria-selected', 'true');
		expect(getCurrentFileList()?.files[getSelectedFileIndex()]?.path).toBe('/books/beta.m4b');
	});

	it('does not sort from the book header while ordering is locked', async () => {
		const first = file('/books/zeta.m4b', 'zeta');
		const second = file('/books/alpha.m4b', 'alpha');
		setCurrentFileList(fileList(first, second));
		setOrderLocked(true);
		const screen = render(FileListIsland);

		await fireEvent.click(screen.getByRole('button', { name: 'File' }));

		expect(getCurrentFileList()?.files.map((item) => item.path)).toEqual([
			'/books/zeta.m4b',
			'/books/alpha.m4b',
		]);
		expect(screen.getByRole('button', { name: 'File' }).closest('th')).toHaveAttribute(
			'aria-sort',
			'none',
		);
	});

	it('activates selection from the row body but keeps Cmd-click toggles local', async () => {
		const openMetadataSurface = vi.fn();
		setMetadataSurfacePresentation({
			open: openMetadataSurface,
			closeWithoutStaging: vi.fn(),
			isOpen: () => false,
		});
		const screen = render(FileListIsland);
		const firstRow = rowFor(screen, 'ready.m4b');

		await fireEvent.click(firstRow);
		await waitFor(() => expect(getSelectedFileIndex()).toBe(0));
		expect(openMetadataSurface).toHaveBeenCalledWith(
			screen.getByRole('button', { name: 'Edit metadata for ready.m4b' }),
		);

		setSelectedFileIndices([]);
		setSelectedIndex(-1);
		await tick();
		openMetadataSurface.mockClear();
		await fireEvent.click(firstRow, { metaKey: true });

		await waitFor(() => expect(firstRow).toHaveAttribute('aria-selected', 'true'));
		expect(openMetadataSurface).not.toHaveBeenCalled();
	});

	it('preserves selected identities and suppresses row activation after a rendered drag reorder', async () => {
		setCurrentFileList(
			fileList(
				file('/books/one.m4b', 'one'),
				file('/books/two.m4b', 'two'),
				file('/books/three.m4b', 'three'),
			),
		);
		setSelectedFileIndices([0, 1]);
		setSelectedIndex(0);
		const openMetadataSurface = vi.fn();
		setMetadataSurfacePresentation({
			open: openMetadataSurface,
			closeWithoutStaging: vi.fn(),
			isOpen: () => false,
		});
		const screen = render(FileListIsland);
		const draggedRow = rowFor(screen, 'one.m4b');
		const dropRow = rowFor(screen, 'three.m4b');
		const dragGrip = draggedRow.querySelector<HTMLElement>('.file-list-reorder-grip');
		if (!dragGrip) throw new Error('Expected rendered file-list drag grip');
		const restoreHitTest = stubElementFromPoint(() => dropRow);

		try {
			await fireEvent.pointerDown(dragGrip, { pointerId: 1, button: 0, clientX: 0, clientY: 0 });
			await fireEvent.pointerMove(window, { pointerId: 1, clientX: 0, clientY: 60 });
			await fireEvent.pointerUp(window, { pointerId: 1 });
			await waitFor(() => {
				expect(getCurrentFileList()?.files.map((item) => item.path)).toEqual([
					'/books/two.m4b',
					'/books/three.m4b',
					'/books/one.m4b',
				]);
			});
		} finally {
			restoreHitTest();
		}

		expect(Array.from(getSelectedFileIndices()).sort()).toEqual([0, 2]);
		expect(getCurrentFileList()?.files[getSelectedFileIndex()]?.path).toBe('/books/one.m4b');
		await fireEvent.click(draggedRow);

		expect(Array.from(getSelectedFileIndices()).sort()).toEqual([0, 2]);
		expect(getCurrentFileList()?.files[getSelectedFileIndex()]?.path).toBe('/books/one.m4b');
		expect(openMetadataSurface).not.toHaveBeenCalled();
	});
});
