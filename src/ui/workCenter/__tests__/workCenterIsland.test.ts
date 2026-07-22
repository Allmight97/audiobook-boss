import { fireEvent, render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it } from 'vitest';
import type { OperationSnapshot } from '../../../types/workRuntime';
import { applyOperationListSnapshot, disposeWorkCenter } from '../state.svelte';
import WorkCenterIsland from '../WorkCenterIsland.svelte';

const nowMs = new Date('2026-07-16T12:00:00').getTime();

function localTime(timestampMs: number): string {
	const timestamp = new Date(timestampMs);
	return [timestamp.getHours(), timestamp.getMinutes(), timestamp.getSeconds()]
		.map((value) => String(value).padStart(2, '0'))
		.join(':');
}

function operation(overrides: Partial<OperationSnapshot> = {}): OperationSnapshot {
	return {
		operationId: 'operation-1',
		sequence: 1,
		kind: 'processingBatch',
		status: 'accepted',
		title: 'The Way of Kings — batch encode',
		createdAtMs: nowMs - 120_000,
		startedAtMs: undefined,
		finishedAtMs: undefined,
		cancellable: true,
		cancelRequested: false,
		lanes: ['encodeCpu'],
		sourceInputIds: ['input-1'],
		progress: {
			stage: 'pending',
			percentage: 0,
			message: 'Queued.',
			currentItemIndex: undefined,
			totalItems: 1,
			bytesDownloaded: undefined,
			bytesTotal: undefined,
			etaSeconds: undefined,
		},
		children: [],
		terminalSummary: undefined,
		warnings: [],
		errors: [],
		logTail: [],
		...overrides,
	};
}

describe('WorkCenterIsland operation cards', () => {
	beforeEach(() => {
		disposeWorkCenter();
		applyOperationListSnapshot({ operations: [] });
	});

	it('renders lower-case statuses, queued position by sequence, terminal time, and the backend log tail', async () => {
		const firstQueued = operation({ operationId: 'queued-1', sequence: 3 });
		const secondQueued = operation({
			operationId: 'queued-2',
			sequence: 2,
			title: 'Second queued',
		});
		const completed = operation({
			operationId: 'completed-1',
			sequence: 1,
			status: 'completed',
			title: 'Completed book',
			finishedAtMs: Date.now() - 2 * 60 * 1000,
			children: [
				{
					childJobId: 'completed-1-child',
					operationId: 'completed-1',
					label: 'Completed book.m4b',
					status: 'completed',
					lane: 'encodeCpu',
					progress: {
						stage: 'complete',
						percentage: 100,
						message: 'Complete.',
						currentItemIndex: undefined,
						totalItems: 1,
						bytesDownloaded: undefined,
						bytesTotal: undefined,
						etaSeconds: undefined,
					},
					sourcePath: undefined,
					inputIndex: 0,
					inputId: 'input-1',
					jobId: 'job-1',
					cancellable: false,
					cancelRequested: false,
					message: 'Complete.',
				},
			],
			logTail: [
				{ timestampMs: new Date('2026-07-16T12:34:56').getTime(), message: 'Started encode' },
				{ timestampMs: new Date('2026-07-16T12:35:00').getTime(), message: 'Finished encode' },
			],
		});
		applyOperationListSnapshot({ operations: [firstQueued, secondQueued, completed] });

		const { container } = render(WorkCenterIsland);
		expect(screen.getAllByText('queued')).toHaveLength(2);
		// FIFO dispatch order: the LOWER sequence runs next and must carry #1
		// even though the list renders newest-first.
		expect(screen.getByText('#1').closest('.op-row')).toHaveTextContent('Second queued');
		expect(screen.getByText('#2').closest('.op-row')).not.toHaveTextContent('Second queued');
		expect(screen.getByText('2m ago')).toBeInTheDocument();
		expect(container.querySelector('.op-card.terminal')).toBeTruthy();

		await fireEvent.click(screen.getByRole('button', { name: 'Expand Completed book' }));
		const log = container.querySelector('.op-log');
		expect(log).toHaveTextContent(
			`${localTime(new Date('2026-07-16T12:34:56').getTime())} Started encode`,
		);
		expect(log).toHaveTextContent(
			`${localTime(new Date('2026-07-16T12:35:00').getTime())} Finished encode`,
		);
		expect(log?.querySelectorAll('b')).toHaveLength(2);

		const track = container.querySelector('[aria-label="Encode progress"]') as HTMLElement;
		expect(track).toHaveAttribute('role', 'progressbar');
		expect(track).toHaveAttribute('aria-valuemin', '0');
		expect(track).toHaveAttribute('aria-valuemax', '100');
		expect(track).toHaveAttribute('aria-valuenow', '100');
	});

	it('does not toggle expansion or preventDefault when Enter/Space targets a Cancel button', async () => {
		applyOperationListSnapshot({ operations: [operation()] });
		render(WorkCenterIsland);
		const row = screen.getByRole('button', {
			name: 'Expand The Way of Kings — batch encode',
		});
		const cancel = screen.getByRole('button', {
			name: 'Cancel The Way of Kings — batch encode',
		});

		expect(await fireEvent.keyDown(cancel, { key: 'Enter' })).toBe(true);
		expect(row).toHaveAttribute('aria-expanded', 'false');
		expect(await fireEvent.keyDown(cancel, { key: ' ' })).toBe(true);
		expect(row).toHaveAttribute('aria-expanded', 'false');
	});

	it('still toggles expansion via Enter/Space on the row itself', async () => {
		applyOperationListSnapshot({ operations: [operation()] });
		render(WorkCenterIsland);
		const row = screen.getByRole('button', {
			name: 'Expand The Way of Kings — batch encode',
		});

		expect(await fireEvent.keyDown(row, { key: 'Enter' })).toBe(false);
		expect(row).toHaveAttribute('aria-expanded', 'true');
		expect(await fireEvent.keyDown(row, { key: ' ' })).toBe(false);
		expect(row).toHaveAttribute('aria-expanded', 'false');
	});
});
