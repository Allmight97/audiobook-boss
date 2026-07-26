import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ProcessingProgressEvent, ProcessingQueueEvent } from '../../../types/events';
import { STAGES } from '../../../types/events';
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

function getJobRows(): string[] {
	return statusPanelViewState.jobItems.map((item) => {
		const percentage =
			typeof item.percentage === 'number' ? ` (${item.percentage.toFixed(1)}%)` : '';
		return `${item.label} • ${item.statusText}${percentage}`;
	});
}

describe('StatusPanel aggregate progress', () => {
	beforeEach(() => {
		setupDom();
		resetStatusPanelViewState();
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.clearAllTimers();
		vi.useRealTimers();
		vi.restoreAllMocks();
	});

	it('computes simple averages across active and completed jobs', () => {
		const controller = new StatusPanelRuntime();
		const snapshot: ProcessingQueueEvent = {
			operation_kind: 'processingBatch',
			items: [
				{ input_index: 0, file_path: '/books/alpha.m4b' },
				{ input_index: 1, file_path: '/books/beta.m4b' },
			],
			max_concurrent: 2,
		};
		const converting: ProcessingProgressEvent = {
			operation_kind: 'processingBatch',
			input_index: 0,
			stage: 'converting',
			percentage: 50,
			message: 'Halfway',
		};
		const completed: ProcessingProgressEvent = {
			operation_kind: 'processingBatch',
			input_index: 1,
			stage: 'completed',
			percentage: 100,
			message: 'Done',
		};

		controller.applyQueueSnapshot(snapshot);
		controller.applyProgress(converting);
		controller.applyProgress(completed);
		vi.advanceTimersByTime(20);

		expect(controller.getCurrentStatus()).toEqual({
			stage: 'converting',
			percentage: 75,
			message: 'Halfway',
		});
		expect(statusPanelViewState.progressPercentage).toBe(75);
		expect(statusPanelViewState.statusText).toBe('Converting');
		expect(getJobRows()).toEqual([
			'alpha.m4b • Converting (50.0%)',
			'beta.m4b • Completed (100.0%)',
		]);
	});

	it('counts queued jobs as zero-progress participants in batch averages', () => {
		const controller = new StatusPanelRuntime();
		controller.applyQueueSnapshot({
			operation_kind: 'processingBatch',
			items: [
				{ input_index: 0, file_path: '/books/alpha.m4b' },
				{ input_index: 1, file_path: '/books/beta.m4b' },
				{ input_index: 2, file_path: '/books/gamma.m4b' },
			],
			max_concurrent: 1,
		});

		controller.applyProgress({
			operation_kind: 'processingBatch',
			input_index: 0,
			stage: 'completed',
			percentage: 100,
			message: 'Done',
		});
		vi.advanceTimersByTime(20);

		expect(controller.getCurrentStatus()).toEqual({
			stage: 'analyzing',
			percentage: 33.3,
			message: 'Queued 2 files',
		});
		expect(statusPanelViewState.progressPercentage).toBe(33.3);
		expect(statusPanelViewState.statusText).toBe('Analyzing');
		expect(getJobRows()).toEqual([
			'alpha.m4b • Completed (100.0%)',
			'beta.m4b • Queued • #2 of 3',
			'gamma.m4b • Queued • #3 of 3',
		]);
	});

	// Regression guard for the discriminated-union refactor. Before `buildStatus`,
	// `flushRender` unconditionally copied `current_file` / `eta_seconds` from
	// `latestProgressEvent` onto the next status — even when the aggregate had
	// already rolled over to a terminal stage. Under the refactored union these
	// fields are type-level impossible on terminal variants; this test pins the
	// runtime behavior so a future regression (e.g. someone reintroduces the
	// copy-unconditional pattern behind a cast) fails loudly.
	it.each([
		STAGES.completed,
		STAGES.skipped,
		STAGES.failed,
		STAGES.cancelled,
	])('does not leak stale currentFile/etaSeconds onto terminal aggregates for %s', (terminalStage) => {
		const controller = new StatusPanelRuntime();
		const snapshot: ProcessingQueueEvent = {
			operation_kind: 'processingBatch',
			items: [{ input_index: 0, file_path: '/books/alpha.m4b' }],
			max_concurrent: 1,
		};

		controller.applyQueueSnapshot(snapshot);

		// Active progress populates `latestProgressEvent` with active-stage
		// fields; a careless flushRender would preserve these onto the terminal
		// status that follows.
		controller.applyProgress({
			operation_kind: 'processingBatch',
			input_index: 0,
			stage: 'converting',
			percentage: 80,
			message: 'Working',
			current_file: '/books/alpha.m4b',
			eta_seconds: 42,
		});

		controller.applyProgress({
			operation_kind: 'processingBatch',
			input_index: 0,
			stage: terminalStage,
			percentage: 100,
			message: 'Done',
		});
		vi.advanceTimersByTime(20);

		const status = controller.getCurrentStatus();
		expect(status).toMatchObject({
			stage: terminalStage === STAGES.skipped ? 'completed' : terminalStage,
		});
		expect(status).not.toHaveProperty('currentFile');
		expect(status).not.toHaveProperty('etaSeconds');
	});
});
