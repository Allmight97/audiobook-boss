import { fireEvent, render, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { AudioFile, FileListInfo } from '../../../types/audio';
import type { OperationSnapshot } from '../../../types/workRuntime';
import { readWorkActivityByInputId } from '../../workCenter';
import { applyOperationSnapshot } from '../../workCenter/state.svelte';
import FileListIsland from '../FileListIsland.svelte';
import { setCurrentFileList, setSelectedFileIndices, setSelectedIndex } from '../state.svelte';

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
	};
}

describe('v3 book table', () => {
	beforeEach(() => {
		setCurrentFileList(
			fileList(file('/books/ready.m4b', 'ready'), file('/books/bad.m4b', 'bad', false)),
		);
		setSelectedFileIndices([]);
		setSelectedIndex(-1);
	});

	afterEach(() => {
		document.documentElement.removeAttribute('data-density');
	});

	it('renders derived activity badges but makes invalid input an error', async () => {
		applyOperationSnapshot(batchSnapshot('ready'));
		const screen = render(FileListIsland, { props: { readWorkActivityByInputId } });

		await waitFor(() => {
			expect(screen.getByText('Running')).toBeInTheDocument();
		});
		expect(screen.getByText('Error')).toHaveAttribute('title', 'Unreadable audio');
	});

	it('renders skipped work as skipped rather than done', async () => {
		const snapshot = batchSnapshot('ready');
		snapshot.children[0] = { ...snapshot.children[0]!, status: 'skipped' };
		applyOperationSnapshot(snapshot);
		const screen = render(FileListIsland, { props: { readWorkActivityByInputId } });

		await waitFor(() => expect(screen.getByText('Skipped')).toBeInTheDocument());
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
});
