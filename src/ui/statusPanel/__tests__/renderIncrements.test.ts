import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderJobList } from '../render';
import * as dom from '../dom';
import type { JobProgress } from '../state';

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
		dom.resetStatusPanelDomCache();
	});

	it('suppresses DOM mutations when payload values are unchanged', async () => {
		const jobProgress = buildJobProgress();
		const queueOrder = ['idx:0'];
		renderJobList(jobProgress, queueOrder, vi.fn());

		const jobList = document.getElementById('job-list')!;
		const mutations: MutationRecord[] = [];
		const observer = new MutationObserver((records) => {
			mutations.push(...records);
		});
		observer.observe(jobList, {
			childList: true,
			subtree: true,
			attributes: true,
			characterData: true,
		});

		renderJobList(cloneJobProgress(jobProgress), [...queueOrder], vi.fn());
		await Promise.resolve();
		observer.disconnect();

		expect(mutations).toHaveLength(0);
	});

	it('reuses keyed rows and only updates changed job values', () => {
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

		const jobList = document.getElementById('job-list')!;
		const row0 = document.getElementById('cancel-idx:0')?.parentElement!;
		const row1 = document.getElementById('cancel-idx:1')?.parentElement!;
		const row0Label = row0.querySelector('span')!;
		const row1Label = row1.querySelector('span')!;

		const updated = cloneJobProgress(jobProgress);
		updated.set('idx:1', {
			...updated.get('idx:1')!,
			percentage: 50,
			lastUpdate: 102,
		});
		renderJobList(updated, queueOrder, vi.fn());

		const nextRow0 = document.getElementById('cancel-idx:0')?.parentElement!;
		const nextRow1 = document.getElementById('cancel-idx:1')?.parentElement!;
		const orderedButtonIds = Array.from(jobList.querySelectorAll('button')).map(
			(button) => button.id,
		);

		expect(nextRow0).toBe(row0);
		expect(nextRow1).toBe(row1);
		expect(orderedButtonIds).toEqual(['cancel-idx:0', 'cancel-idx:1']);
		expect(row0.querySelector('span')).toBe(row0Label);
		expect(row1.querySelector('span')).toBe(row1Label);
		expect(row0Label.textContent).toBe('alpha.m4b • Queued • #1 of 2');
		expect(row1Label.textContent).toBe('beta.m4b • Writing Metadata (50.0%)');
	});

	it('preserves cancel and terminal behavior while rows stay keyed', () => {
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

		const button = document.getElementById('cancel-idx:0') as HTMLButtonElement;
		button.click();
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

		const sameButton = document.getElementById('cancel-idx:0') as HTMLButtonElement;
		expect(sameButton).toBe(button);
		sameButton.click();
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

		const terminalButton = document.getElementById('cancel-idx:0') as HTMLButtonElement;
		expect(terminalButton.disabled).toBe(true);
		terminalButton.click();
		expect(cancelSpyB).toHaveBeenCalledTimes(1);
	});

	it('clears cached rows when the list is emptied', () => {
		const jobProgress = buildJobProgress();
		const queueOrder = ['idx:0'];
		renderJobList(jobProgress, queueOrder, vi.fn());

		const jobList = document.getElementById('job-list')!;
		expect(jobList.childElementCount).toBe(1);

		renderJobList(new Map(), [], vi.fn());
		expect(jobList.childElementCount).toBe(0);
		expect(jobList.innerHTML).toBe('');

		renderJobList(jobProgress, queueOrder, vi.fn());
		expect(jobList.childElementCount).toBe(1);
	});
});
