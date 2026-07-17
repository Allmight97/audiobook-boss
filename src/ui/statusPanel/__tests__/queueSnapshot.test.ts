import { beforeEach, describe, expect, it } from 'vitest';
import type { ProcessingProgressEvent, ProcessingQueueEvent } from '../../../types/events';
import { StatusPanelRuntime } from '../controller';
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

		expect(statusPanelViewState.foregroundJobLabel).toBe('alpha.m4b');
		expect(statusPanelViewState.hasCancellableForegroundJob).toBe(false);
		expect(statusPanelViewState.statusText).toBe('Analyzing');
		expect(statusPanelViewState.stepText).toBe('Current Step: Queued 2 files');
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

		expect(statusPanelViewState.foregroundJobLabel).toBe('Processing');
		expect(statusPanelViewState.stepText).toBe('Current Step: working');

		controller.applyQueueSnapshot(snapshot);

		expect(statusPanelViewState.foregroundJobLabel).toBe('gamma.m4b');
		expect(statusPanelViewState.stepText).toBe('Current Step: Queued 3 files');
		const status = controller.getCurrentStatus();
		expect(status).toEqual({
			stage: 'analyzing',
			percentage: 0,
			message: 'Queued 3 files',
		});
		expect(status).not.toHaveProperty('currentFile');
		expect(status).not.toHaveProperty('etaSeconds');
	});
});
