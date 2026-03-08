import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ProcessingProgressEvent, ProcessingQueueEvent } from '../../types/events';
import { StatusPanelController } from './controller';
import { resetStatusPanelViewState, statusPanelViewState } from './viewState.svelte';

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
    <div id="output-dir-text"></div>
  `;
}

function getJobRows(): string[] {
	return statusPanelViewState.jobItems.map((item) => {
		const percentage =
			typeof item.percentage === 'number' ? ` (${item.percentage.toFixed(1)}%)` : '';
		return `${item.label} • ${item.statusText}${percentage}`;
	});
}

function getStepText(): string {
	return statusPanelViewState.stepText;
}

function getStatusText(): string {
	return statusPanelViewState.statusText;
}

function getPercentageText(): string {
	return `${statusPanelViewState.progressPercentage.toFixed(1)}%`;
}

describe('StatusPanel progress throttling', () => {
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

	it('throttles rapid non-terminal progress events', () => {
		const controller = new StatusPanelController();
		const evt: ProcessingProgressEvent = {
			job_id: 'job-1',
			stage: 'converting',
			percentage: 10,
			message: 'ten',
		};

		controller.applyProgress(evt);
		vi.advanceTimersByTime(20);
		expect(getPercentageText()).toBe('10.0%');
		expect(getStepText()).toBe('Current Step: ten');

		const evt2: ProcessingProgressEvent = { ...evt, percentage: 20, message: 'twenty' };
		controller.applyProgress(evt2);
		vi.advanceTimersByTime(20);
		expect(getPercentageText()).toBe('10.0%');
		expect(getStepText()).toBe('Current Step: ten');

		vi.advanceTimersByTime(1050);
		const evt3: ProcessingProgressEvent = { ...evt, percentage: 30, message: 'thirty' };
		controller.applyProgress(evt3);
		vi.advanceTimersByTime(20);

		expect(getPercentageText()).toBe('30.0%');
		expect(getStepText()).toBe('Current Step: thirty');
		expect(getJobRows()[0]).toContain('(30.0%)');
	});

	it('does not throttle different jobs within the same window', () => {
		const controller = new StatusPanelController();
		const evt1: ProcessingProgressEvent = {
			job_id: 'job-1',
			stage: 'converting',
			percentage: 10,
			message: 'ten',
		};
		const evt2: ProcessingProgressEvent = {
			job_id: 'job-2',
			stage: 'converting',
			percentage: 20,
			message: 'twenty',
		};

		controller.applyProgress(evt1);
		controller.applyProgress(evt2);
		vi.advanceTimersByTime(20);

		expect(getStatusText()).toBe('Converting');
		expect(getStepText()).toBe('Current Step: Processing 2 files');
		expect(controller.getCurrentStatus().percentage).toBe(15);
		expect(getJobRows()).toEqual(
			expect.arrayContaining([
				expect.stringContaining('(10.0%)'),
				expect.stringContaining('(20.0%)'),
			]),
		);
	});

	it('does not throttle terminal events inside the throttle window', () => {
		const controller = new StatusPanelController();
		const queueSnapshot: ProcessingQueueEvent = {
			items: [
				{ input_index: 0, file_path: '/books/alpha.m4b' },
				{ input_index: 1, file_path: '/books/beta.m4b' },
			],
			max_concurrent: 2,
		};
		const progress: ProcessingProgressEvent = {
			input_index: 0,
			stage: 'converting',
			percentage: 12,
			message: 'processing',
		};
		const terminal: ProcessingProgressEvent = {
			...progress,
			stage: 'completed',
			percentage: 100,
			message: 'done',
		};

		controller.applyQueueSnapshot(queueSnapshot);
		controller.applyProgress(progress);
		vi.advanceTimersByTime(20);
		controller.applyProgress(terminal);

		expect(getJobRows()[0]).toBe('alpha.m4b • Completed (100.0%)');
		expect(getStatusText()).toBe('Analyzing');
		expect(getStepText()).toBe('Current Step: Queued 1 file');
		expect(getPercentageText()).toBe('100.0%');
	});
});
