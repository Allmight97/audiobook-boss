import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import * as dom from './dom';
import { StatusPanel } from './logic';

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
	return Array.from(document.querySelectorAll<HTMLElement>('#job-list span')).map(
		(node) => node.textContent ?? '',
	);
}

function getStepText(): string {
	return (document.getElementById('step-text') as HTMLElement).textContent ?? '';
}

function getStatusText(): string {
	return (document.getElementById('status-text') as HTMLElement).textContent ?? '';
}

function getPercentageText(): string {
	return (document.getElementById('percentage-processed') as HTMLElement).textContent ?? '';
}

describe('StatusPanel progress throttling', () => {
	beforeEach(() => {
		setupDom();
		dom.resetStatusPanelDomCache();
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.clearAllTimers();
		vi.useRealTimers();
		vi.restoreAllMocks();
	});

	it('throttles rapid non-terminal progress events', () => {
		const panel = new StatusPanel();

		const evt = {
			job_id: 'job-1',
			stage: 'converting',
			percentage: 10,
			message: 'ten',
		} as any;

		panel.updateProgress(evt);
		vi.advanceTimersByTime(20);
		expect(getPercentageText()).toBe('10.0%');
		expect(getStepText()).toBe('Current Step: ten');

		const evt2 = { ...evt, percentage: 20, message: 'twenty' };
		panel.updateProgress(evt2);
		vi.advanceTimersByTime(20);
		expect(getPercentageText()).toBe('10.0%');
		expect(getStepText()).toBe('Current Step: ten');

		vi.advanceTimersByTime(1050);
		const evt3 = { ...evt, percentage: 30, message: 'thirty' };
		panel.updateProgress(evt3);
		vi.advanceTimersByTime(20);

		expect(getPercentageText()).toBe('30.0%');
		expect(getStepText()).toBe('Current Step: thirty');
		expect(getJobRows()[0]).toContain('(30.0%)');
	});

	it('does not throttle different jobs within the same window', () => {
		const panel = new StatusPanel();

		const evt1 = {
			job_id: 'job-1',
			stage: 'converting',
			percentage: 10,
			message: 'ten',
		} as any;

		const evt2 = {
			job_id: 'job-2',
			stage: 'converting',
			percentage: 20,
			message: 'twenty',
		} as any;

		panel.updateProgress(evt1);
		panel.updateProgress(evt2);
		vi.advanceTimersByTime(20);

		expect(getStatusText()).toBe('Converting');
		expect(getStepText()).toBe('Current Step: Processing 2 files');
		expect(panel.getCurrentStatus().percentage).toBe(15);
		expect(getJobRows()).toEqual(
			expect.arrayContaining([
				expect.stringContaining('(10.0%)'),
				expect.stringContaining('(20.0%)'),
			]),
		);
	});

	it('does not throttle terminal events inside the throttle window', () => {
		const panel = new StatusPanel();

		(panel as any).handleQueueSnapshot({
			items: [
				{ input_index: 0, file_path: '/books/alpha.m4b' },
				{ input_index: 1, file_path: '/books/beta.m4b' },
			],
			max_concurrent: 2,
		});

		const progress = {
			input_index: 0,
			stage: 'converting',
			percentage: 12,
			message: 'processing',
		} as any;
		panel.updateProgress(progress);
		vi.advanceTimersByTime(20);

		const terminal = {
			...progress,
			stage: 'completed',
			percentage: 100,
			message: 'done',
		};
		panel.updateProgress(terminal);

		expect(getJobRows()[0]).toBe('alpha.m4b • Completed (100.0%)');
		expect(getStatusText()).toBe('Analyzing');
		expect(getStepText()).toBe('Current Step: Queued 1 file');
		expect(getPercentageText()).toBe('100.0%');
	});
});
