import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ProcessingProgressEvent, ProcessingQueueEvent } from '../../../types/events';
import { StatusPanelRuntime } from '../runtime';
import { SINGLE_COMPLETION_HOLD_MS } from '../domain/stateMachine';
import { resetStatusPanelViewState, getStatusView } from '../view';

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

function getJobRows(): string[] {
	return getStatusView().jobItems.map((item) => {
		const percentage =
			typeof item.percentage === 'number' ? ` (${item.percentage.toFixed(1)}%)` : '';
		return `${item.label} • ${item.statusText}${percentage}`;
	});
}

async function flushRenderFrame(): Promise<void> {
	await new Promise<void>((resolve) => {
		requestAnimationFrame(() => resolve());
	});
}

describe('StatusPanel queue snapshot', () => {
	beforeEach(() => {
		setupDom();
		resetStatusPanelViewState();
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it('initializes queued items in order', () => {
		const controller = new StatusPanelRuntime();
		const snapshot: ProcessingQueueEvent = {
			operation_kind: 'processingBatch',
			items: [
				{ input_index: 0, file_path: '/books/alpha.m4b' },
				{ input_index: 1, file_path: '/books/beta.m4b' },
			],
			max_concurrent: 2,
		};

		controller.applyQueueSnapshot(snapshot);

		expect(getJobRows()).toEqual(['alpha.m4b • Queued • #1 of 2', 'beta.m4b • Queued • #2 of 2']);
		expect(getStatusView().statusText).toBe('Analyzing');
		expect(getStatusView().stepText).toBe('Current Step: Queued 2 files');
		const status = controller.getCurrentStatus();
		expect(status).toEqual({
			stage: 'analyzing',
			percentage: 0,
			message: 'Queued 2 files',
		});
		expect(status).not.toHaveProperty('currentFile');
		expect(status).not.toHaveProperty('etaSeconds');
	});

	it('applies queue snapshot order and labels after early progress arrives', async () => {
		const controller = new StatusPanelRuntime();
		const earlyProgress: ProcessingProgressEvent = {
			operation_kind: 'processingBatch',
			input_index: 1,
			stage: 'converting',
			percentage: 45,
			message: 'working',
		};
		const snapshot: ProcessingQueueEvent = {
			operation_kind: 'processingBatch',
			items: [
				{ input_index: 2, file_path: '/books/gamma.m4b' },
				{ input_index: 1, file_path: '/books/beta.m4b' },
				{ input_index: 0, file_path: '/books/alpha.m4b' },
			],
			max_concurrent: 2,
		};

		controller.applyProgress(earlyProgress);
		await flushRenderFrame();

		expect(getJobRows()).toHaveLength(1);
		expect(getJobRows()[0]).toContain('Converting (45.0%)');
		expect(getStatusView().stepText).toBe('Current Step: working');

		controller.applyQueueSnapshot(snapshot);

		expect(getJobRows()).toEqual([
			'gamma.m4b • Queued • #1 of 3',
			'beta.m4b • Queued • #2 of 3',
			'alpha.m4b • Queued • #3 of 3',
		]);
		expect(getStatusView().stepText).toBe('Current Step: Queued 3 files');
		const status = controller.getCurrentStatus();
		expect(status).toEqual({
			stage: 'analyzing',
			percentage: 0,
			message: 'Queued 3 files',
		});
		expect(status).not.toHaveProperty('currentFile');
		expect(status).not.toHaveProperty('etaSeconds');
	});

	it('does not resurrect a locally cancelled run when a late queue snapshot arrives', () => {
		vi.useFakeTimers();
		const controller = new StatusPanelRuntime();
		controller.applyProgress({
			operation_kind: 'processingBatch',
			input_index: 0,
			job_id: 'job-0',
			stage: 'converting',
			percentage: 25,
			message: 'Converting',
		});
		controller.requestCancelAll();

		expect(getStatusView().jobItems.map((item) => item.status)).toEqual(['cancelled']);
		expect(getStatusView().statusText).toBe('Cancelled');

		controller.applyQueueSnapshot({
			operation_kind: 'processingBatch',
			items: [
				{ input_index: 0, file_path: '/books/a.m4b' },
				{ input_index: 1, file_path: '/books/b.m4b' },
			],
			max_concurrent: 2,
		});

		expect(getStatusView().jobItems.map((item) => item.status)).toEqual(['cancelled']);
		expect(getStatusView().jobItems).toHaveLength(1);
		expect(getStatusView().statusText).toBe('Cancelled');

		vi.advanceTimersByTime(SINGLE_COMPLETION_HOLD_MS);

		expect(getStatusView().jobItems).toEqual([]);
		expect(getStatusView().isProcessing).toBe(false);
		expect(getStatusView().statusText).toBe('Idle');
	});
});
