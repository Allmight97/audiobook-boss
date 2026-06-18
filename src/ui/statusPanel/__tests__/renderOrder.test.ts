import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderJobList as renderJobListView } from '../render';
import type { JobProgress } from '../state';
import * as viewState from '../viewState.svelte';

vi.mock('../viewState.svelte', () => ({
	pushTransientStatusMessage: vi.fn(),
	setStatusPanelCancelAllPending: vi.fn(),
	setStatusPanelConcurrencyText: vi.fn(),
	setStatusPanelCoverArtDataUrl: vi.fn(),
	setStatusPanelIsProcessing: vi.fn(),
	setStatusPanelJobItems: vi.fn(),
	setStatusPanelProgressPercentage: vi.fn(),
	setStatusPanelStatusText: vi.fn(),
	setStatusPanelStepColor: vi.fn(),
	setStatusPanelStepText: vi.fn(),
	showError: vi.fn(),
	showInfo: vi.fn(),
	showSuccess: vi.fn(),
}));

describe('renderJobList order and queue status text', () => {
	const renderListMock = vi.mocked(viewState.setStatusPanelJobItems);

	beforeEach(() => {
		renderListMock.mockClear();
	});

	it('keeps queue order first and appends extras by latest update', () => {
		const jobProgress = new Map<string, JobProgress>([
			[
				'idx:1',
				{
					inputIndex: 1,
					label: 'second-in-queue.m4b',
					status: 'queued',
					percentage: 0,
					message: 'Queued',
					lastUpdate: 100,
				},
			],
			[
				'idx:0',
				{
					inputIndex: 0,
					label: 'first-in-queue.m4b',
					status: 'queued',
					percentage: 0,
					message: 'Queued',
					lastUpdate: 200,
				},
			],
			[
				'extra:processing',
				{
					inputIndex: 8,
					jobId: 'job-extra-processing',
					label: 'extra-processing.m4b',
					status: 'processing',
					stage: 'writing',
					percentage: 64.2,
					message: 'Writing metadata',
					lastUpdate: 500,
				},
			],
			[
				'extra:queued',
				{
					inputIndex: 9,
					label: 'extra-queued.m4b',
					status: 'queued',
					percentage: 0,
					message: 'Queued',
					lastUpdate: 400,
				},
			],
			[
				'extra:completed',
				{
					inputIndex: 10,
					label: 'extra-completed.m4b',
					status: 'completed',
					percentage: 100,
					message: 'Done',
					lastUpdate: 300,
				},
			],
		]);

		renderJobListView(jobProgress, ['idx:0', 'idx:1'], vi.fn());

		expect(renderListMock).toHaveBeenCalledTimes(1);
		const [jobs] = renderListMock.mock.calls[0];

		expect(jobs.map((job) => job.key)).toEqual([
			'idx:0',
			'idx:1',
			'extra:processing',
			'extra:queued',
			'extra:completed',
		]);

		expect(jobs.map((job) => job.statusText)).toEqual([
			'Queued • #1 of 2',
			'Queued • #2 of 2',
			'Writing Metadata',
			'Queued',
			'Completed',
		]);
	});
});
