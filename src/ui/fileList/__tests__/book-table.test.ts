import { fireEvent, render, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AudioFile, FileListInfo } from '../../../types/audio';
import type { OperationSnapshot } from '../../../types/workRuntime';
import { readWorkActivityByInputId } from '../../workCenter';
import { applyOperationSnapshot } from '../../workCenter/state.svelte';
import FileListIsland from '../FileListIsland.svelte';
import {
	getCurrentFileList,
	getSelectedFileIndex,
	setCurrentFileList,
	setOrderLocked,
	setSelectedFileIndices,
	setSelectedIndex,
} from '../state.svelte';
import { cacheMetadataForFile, clearMetadataSession } from '../../metadataSession';
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
		document.documentElement.removeAttribute('data-density');
		setMetadataSurfacePresentation(null);
	});

	it('re-renders titles and authors when metadata caches after first paint', async () => {
		const screen = render(FileListIsland, { props: { readWorkActivityByInputId } });

		// First paint precedes the async backend metadata read: basename fallback.
		expect(screen.getByText('ready.m4b')).toBeInTheDocument();

		cacheMetadataForFile('/books/ready.m4b', {
			title: 'The Way of Kings',
			artist: 'Brandon Sanderson',
		});
		await tick();

		expect(screen.getByText('The Way of Kings')).toBeInTheDocument();
		expect(screen.getByText('Brandon Sanderson')).toBeInTheDocument();
		expect(screen.queryByText('ready.m4b')).toBeNull();
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

	it('marks select-all indeterminate for a partial selection', async () => {
		const screen = render(FileListIsland);
		setSelectedFileIndices([0]);
		setSelectedIndex(0);
		await tick();

		expect(screen.getByRole('checkbox', { name: 'Select all files' })).toHaveProperty(
			'indeterminate',
			true,
		);
	});

	it('uses a group for keyboard navigation and exposes selection on row checkboxes', async () => {
		const screen = render(FileListIsland);
		setSelectedFileIndices([0]);
		setSelectedIndex(0);
		await tick();

		expect(screen.getByRole('group', { name: 'Audio files' })).not.toHaveAttribute(
			'aria-multiselectable',
		);
		expect(screen.getByRole('checkbox', { name: 'Select ready.m4b' })).toBeChecked();
	});

	it('exposes reorder shortcuts and changes the row grip when ordering locks', async () => {
		const screen = render(FileListIsland);
		const keyboardGroup = screen.getByRole('group', { name: 'Audio files' });
		const row = screen.getByRole('checkbox', { name: 'Select ready.m4b' }).closest('tr');
		if (!row) throw new Error('Expected a file-list row');
		const grip = row.querySelector('.file-list-reorder-grip');

		expect(keyboardGroup).toHaveAttribute('aria-keyshortcuts', 'Alt+ArrowUp Alt+ArrowDown');
		expect(grip).toHaveAttribute(
			'title',
			'Drag to reorder. Move the selected file with Alt+ArrowUp or Alt+ArrowDown.',
		);
		expect(row).toHaveAttribute('draggable', 'true');

		setOrderLocked(true);
		await tick();

		expect(row).toHaveClass('order-locked');
		expect(row).toHaveAttribute('draggable', 'false');
		expect(grip).toHaveTextContent('⠿');
		expect(grip).toHaveAttribute('title', 'File order is locked');
	});

	it('hides comfortable-only columns at compact density', async () => {
		const screen = render(FileListIsland);
		document.documentElement.dataset.density = 'compact';
		await tick();

		expect(screen.getByText('Size')).toHaveClass('file-list-comfortable-only');
	});

	it('select-all checkbox selects all rows', async () => {
		const screen = render(FileListIsland);
		await fireEvent.click(screen.getByRole('checkbox', { name: 'Select all files' }));

		await waitFor(() => {
			expect(screen.getAllByRole('checkbox', { name: /Select / })).toHaveLength(3);
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
			expect(screen.getByRole('checkbox', { name: 'Select one.m4b' })).toBeChecked();
			expect(screen.getByRole('checkbox', { name: 'Select two.m4b' })).toBeChecked();
			expect(screen.getByRole('checkbox', { name: 'Select three.m4b' })).toBeChecked();
		});
	});

	it('sorts by displayed title with ascending then descending header state', async () => {
		const zeta = file('/books/zeta.m4b', 'zeta');
		const alpha = file('/books/alpha.m4b', 'alpha');
		setCurrentFileList(fileList(zeta, alpha));
		cacheMetadataForFile(zeta.path, { title: 'A displayed title' });
		cacheMetadataForFile(alpha.path, { title: 'Z displayed title' });
		const screen = render(FileListIsland);
		const header = screen.getByRole('button', { name: 'Book' });

		expect(header.closest('th')).toHaveAttribute('aria-sort', 'none');
		await fireEvent.click(header);
		await waitFor(() => {
			expect(getCurrentFileList()?.files.map((item) => item.path)).toEqual([
				'/books/zeta.m4b',
				'/books/alpha.m4b',
			]);
		});
		expect(header.closest('th')).toHaveAttribute('aria-sort', 'ascending');

		await fireEvent.click(header);
		await waitFor(() => {
			expect(getCurrentFileList()?.files.map((item) => item.path)).toEqual([
				'/books/alpha.m4b',
				'/books/zeta.m4b',
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

		await fireEvent.click(screen.getByRole('button', { name: 'Book' }));
		await waitFor(() => {
			expect(getCurrentFileList()?.files.map((item) => item.path)).toEqual([
				'/books/alpha.m4b',
				'/books/beta.m4b',
				'/books/zeta.m4b',
			]);
		});

		expect(screen.getByRole('checkbox', { name: 'Select beta.m4b' })).toBeChecked();
		expect(screen.getByRole('checkbox', { name: 'Select zeta.m4b' })).toBeChecked();
		expect(getCurrentFileList()?.files[getSelectedFileIndex()]?.path).toBe('/books/beta.m4b');
	});

	it('does not sort from the book header while ordering is locked', async () => {
		const first = file('/books/zeta.m4b', 'zeta');
		const second = file('/books/alpha.m4b', 'alpha');
		setCurrentFileList(fileList(first, second));
		setOrderLocked(true);
		const screen = render(FileListIsland);

		await fireEvent.click(screen.getByRole('button', { name: 'Book' }));

		expect(getCurrentFileList()?.files.map((item) => item.path)).toEqual([
			'/books/zeta.m4b',
			'/books/alpha.m4b',
		]);
		expect(screen.getByRole('button', { name: 'Book' }).closest('th')).toHaveAttribute(
			'aria-sort',
			'none',
		);
	});

	it('activates selection from the row body but keeps checkbox clicks local', async () => {
		const openMetadataSurface = vi.fn();
		setMetadataSurfacePresentation({
			open: openMetadataSurface,
			closeWithoutStaging: vi.fn(),
			isOpen: () => false,
		});
		const screen = render(FileListIsland);
		const firstRow = screen.getByRole('checkbox', { name: 'Select ready.m4b' }).closest('tr');
		if (!firstRow) throw new Error('Expected a file-list row');

		await fireEvent.click(firstRow);
		await waitFor(() => expect(getSelectedFileIndex()).toBe(0));
		expect(openMetadataSurface).toHaveBeenCalledWith(
			screen.getByRole('button', { name: 'Edit metadata for ready.m4b' }),
		);

		setSelectedFileIndices([]);
		setSelectedIndex(-1);
		await tick();
		openMetadataSurface.mockClear();
		await fireEvent.click(screen.getByRole('checkbox', { name: 'Select ready.m4b' }));

		expect(screen.getByRole('checkbox', { name: 'Select ready.m4b' })).toBeChecked();
		expect(screen.getByRole('button', { name: 'Edit metadata for ready.m4b' })).toHaveAttribute(
			'aria-pressed',
			'true',
		);
		expect(openMetadataSurface).not.toHaveBeenCalled();
	});
});
