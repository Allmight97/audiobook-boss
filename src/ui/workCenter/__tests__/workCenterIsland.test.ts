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
		const secondQueued = operation({ operationId: 'queued-2', sequence: 2, title: 'Second queued' });
		const completed = operation({
			operationId: 'completed-1',
			sequence: 1,
			status: 'completed',
			title: 'Completed book',
			finishedAtMs: Date.now() - 2 * 60 * 1000,
			logTail: [
				{ timestampMs: new Date('2026-07-16T12:34:56').getTime(), message: 'Started encode' },
				{ timestampMs: new Date('2026-07-16T12:35:00').getTime(), message: 'Finished encode' },
			],
		});
		applyOperationListSnapshot({ operations: [firstQueued, secondQueued, completed] });

		const { container } = render(WorkCenterIsland);
		expect(screen.getAllByText('queued')).toHaveLength(2);
		expect(screen.getByText('#2')).toBeInTheDocument();
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
	});
});
