import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderJobList } from '../render';
import type { JobProgress } from '../state';
import { resetStatusPanelViewState, statusPanelViewState } from '../viewState.svelte';

function setupDom() {
	document.body.innerHTML = `
    <div id="progress-bar"></div>
    <div id="percentage-processed"></div>
    <div id="status-text"></div>
    <div id="step-text"></div>
    <div id="concurrency-status"></div>
    <button id="process-button"></button>
    <button id="cancel-all-button"></button>
    <div class="art-thumbnail"></div>
    <div id="job-list"></div>
    <select id="max-concurrent-select"></select>
  `;
}

function buildJobProgress(): Map<string, JobProgress> {
	return new Map([
		[
			'idx:0',
			{
				inputIndex: 0,
				label: 'book.m4b',
				status: 'queued',
				percentage: 0,
				message: 'Queued',
				lastUpdate: 123,
			},
		],
	]);
}

function cloneJobProgress(source: Map<string, JobProgress>): Map<string, JobProgress> {
	return new Map(Array.from(source.entries(), ([key, value]) => [key, { ...value }]));
}

describe('renderJobList incremental updates', () => {
	beforeEach(() => {
		setupDom();
		resetStatusPanelViewState();
	});

	it('keeps reactive job items stable when payload values are unchanged', () => {
		const jobProgress = buildJobProgress();
		const queueOrder = ['idx:0'];
		renderJobList(jobProgress, queueOrder, vi.fn());
		const firstSnapshot = JSON.stringify(statusPanelViewState.jobItems);

		renderJobList(cloneJobProgress(jobProgress), [...queueOrder], vi.fn());
		const secondSnapshot = JSON.stringify(statusPanelViewState.jobItems);
		expect(secondSnapshot).toBe(firstSnapshot);
	});

	it('keeps ordered keyed items and updates only changed values', () => {
		const jobProgress = new Map<string, JobProgress>([
			[
				'idx:0',
				{
					inputIndex: 0,
					label: 'alpha.m4b',
					status: 'queued',
					percentage: 0,
					message: 'Queued',
					lastUpdate: 100,
				},
			],
			[
				'idx:1',
				{
					inputIndex: 1,
					jobId: 'job-1',
					label: 'beta.m4b',
					status: 'processing',
					stage: 'writing',
					percentage: 25,
					message: 'Writing metadata',
					lastUpdate: 101,
				},
			],
		]);
		const queueOrder = ['idx:0', 'idx:1'];

		renderJobList(jobProgress, queueOrder, vi.fn());

		const updated = cloneJobProgress(jobProgress);
		updated.set('idx:1', {
			...updated.get('idx:1')!,
			percentage: 50,
			lastUpdate: 102,
		});
		renderJobList(updated, queueOrder, vi.fn());

		expect(statusPanelViewState.jobItems.map((item) => item.key)).toEqual(['idx:0', 'idx:1']);
		expect(statusPanelViewState.jobItems[0]).toMatchObject({
			label: 'alpha.m4b',
			status: 'queued',
			statusText: 'Queued • #1 of 2',
		});
		expect(statusPanelViewState.jobItems[1]).toMatchObject({
			label: 'beta.m4b',
			status: 'processing',
			stage: 'writing',
			statusText: 'Writing Metadata',
			percentage: 50,
		});
	});

	it('preserves cancel and terminal behavior in reactive items', () => {
		const queueOrder = ['idx:0'];
		const cancelSpyA = vi.fn();
		const cancelSpyB = vi.fn();

		const processing = new Map<string, JobProgress>([
			[
				'idx:0',
				{
					inputIndex: 0,
					jobId: 'job-1',
					label: 'book.m4b',
					status: 'processing',
					stage: 'converting',
					percentage: 40,
					message: 'Converting',
					lastUpdate: 1,
				},
			],
		]);
		renderJobList(processing, queueOrder, cancelSpyA);

		const firstItem = statusPanelViewState.jobItems[0];
		firstItem.onCancel?.(firstItem.cancelId as string);
		expect(cancelSpyA).toHaveBeenCalledTimes(1);
		expect(cancelSpyA).toHaveBeenCalledWith('job-1');

		const processingUpdated = cloneJobProgress(processing);
		processingUpdated.set('idx:0', {
			...processingUpdated.get('idx:0')!,
			jobId: 'job-2',
			percentage: 65,
			lastUpdate: 2,
		});
		renderJobList(processingUpdated, queueOrder, cancelSpyB);

		const secondItem = statusPanelViewState.jobItems[0];
		secondItem.onCancel?.(secondItem.cancelId as string);
		expect(cancelSpyA).toHaveBeenCalledTimes(1);
		expect(cancelSpyB).toHaveBeenCalledTimes(1);
		expect(cancelSpyB).toHaveBeenCalledWith('job-2');

		const completed = cloneJobProgress(processingUpdated);
		completed.set('idx:0', {
			...completed.get('idx:0')!,
			status: 'completed',
			stage: 'completed',
			percentage: 100,
			message: 'Done',
			lastUpdate: 3,
		});
		renderJobList(completed, queueOrder, cancelSpyB);

		const terminalItem = statusPanelViewState.jobItems[0];
		expect(terminalItem.canCancel).toBe(false);
		terminalItem.onCancel?.(terminalItem.cancelId as string);
		expect(cancelSpyB).toHaveBeenCalledTimes(1);
	});

	it('clears reactive rows when the list is emptied', () => {
		const jobProgress = buildJobProgress();
		const queueOrder = ['idx:0'];
		renderJobList(jobProgress, queueOrder, vi.fn());
		expect(statusPanelViewState.jobItems).toHaveLength(1);

		renderJobList(new Map(), [], vi.fn());
		expect(statusPanelViewState.jobItems).toHaveLength(0);

		renderJobList(jobProgress, queueOrder, vi.fn());
		expect(statusPanelViewState.jobItems).toHaveLength(1);
	});
});
