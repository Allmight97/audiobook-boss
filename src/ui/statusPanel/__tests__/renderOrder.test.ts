import { beforeEach, describe, expect, it } from 'vitest';
import { renderTransportSummary } from '../render';
import type { JobProgress } from '../state';
import { resetStatusPanelViewState, statusPanelViewState } from '../viewState.svelte';

describe('transport summary projection', () => {
	beforeEach(() => {
		resetStatusPanelViewState();
	});

	it('uses the first processing job in queue order and exposes cancellability only for a real job id', () => {
		const jobProgress = new Map<string, JobProgress>([
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
				'idx:1',
				{
					inputIndex: 1,
					jobId: 'job-1',
					label: 'first-processing.m4b',
					status: 'processing',
					percentage: 64.2,
					message: 'Writing metadata',
					lastUpdate: 100,
				},
			],
			[
				'extra:processing',
				{
					inputIndex: 8,
					jobId: 'job-extra-processing',
					label: 'later-processing.m4b',
					status: 'processing',
					percentage: 80,
					message: 'Writing metadata',
					lastUpdate: 500,
				},
			],
		]);

		renderTransportSummary(jobProgress, ['idx:0', 'idx:1']);

		expect(statusPanelViewState.foregroundJobLabel).toBe('first-processing.m4b');
		expect(statusPanelViewState.hasCancellableForegroundJob).toBe(true);
	});

	it('falls back to the first ordered job and clears both facts for an empty queue', () => {
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
		]);

		renderTransportSummary(jobProgress, ['idx:0', 'idx:1']);
		expect(statusPanelViewState.foregroundJobLabel).toBe('first-in-queue.m4b');
		expect(statusPanelViewState.hasCancellableForegroundJob).toBe(false);

		renderTransportSummary(new Map(), []);
		expect(statusPanelViewState.foregroundJobLabel).toBeNull();
		expect(statusPanelViewState.hasCancellableForegroundJob).toBe(false);
	});
});
