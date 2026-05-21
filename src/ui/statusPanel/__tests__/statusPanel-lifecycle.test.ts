import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/svelte';
import JobControlsIsland from '../../jobControls/JobControlsIsland.svelte';
import { tauriClient } from '../../../lib/tauri/client';
import { STAGES } from '../../../types/events';
import {
	handleMaxConcurrentSelectionChange,
	handleMergeModeChange,
	initJobControls,
	setJobControlsEnabled,
	setJobTypeSelection,
} from '../../jobControls';
import * as feedback from '../feedback';
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
  `;
	render(JobControlsIsland, {
		onMergeModeChange: handleMergeModeChange,
		onMaxConcurrentSelectionChange: handleMaxConcurrentSelectionChange,
	});
}

function seedDisabledControls() {
	setJobControlsEnabled(false);
}

function assertControlsEnabled() {
	const mergeToggle = document.getElementById('merge-mode-toggle') as HTMLInputElement;
	const maxConcurrent = document.getElementById('max-concurrent-select') as HTMLSelectElement;
	expect(mergeToggle.disabled).toBe(false);
	expect(maxConcurrent.disabled).toBe(false);
	expect(mergeToggle.style.opacity).toBe('1');
	expect(maxConcurrent.style.opacity).toBe('1');
}

function assertControlsDisabled() {
	const mergeToggle = document.getElementById('merge-mode-toggle') as HTMLInputElement;
	const maxConcurrent = document.getElementById('max-concurrent-select') as HTMLSelectElement;
	expect(mergeToggle.disabled).toBe(true);
	expect(maxConcurrent.disabled).toBe(true);
}

function getStepText(): string {
	return statusPanelViewState.stepText;
}

function getJobRows(): string[] {
	return statusPanelViewState.jobItems.map((item) => {
		const percentage =
			typeof item.percentage === 'number' ? ` (${item.percentage.toFixed(1)}%)` : '';
		return `${item.label} • ${item.statusText}${percentage}`;
	});
}

async function flushAsync(): Promise<void> {
	await Promise.resolve();
	await Promise.resolve();
}

describe('StatusPanel lifecycle', () => {
	beforeEach(async () => {
		setupDom();
		resetStatusPanelViewState();
		vi.useFakeTimers();
		vi.spyOn(tauriClient, 'setMaxConcurrentJobs').mockResolvedValue(4);
		setJobTypeSelection('batch');
		initJobControls();
		await flushAsync();
	});

	afterEach(() => {
		vi.clearAllTimers();
		vi.useRealTimers();
		vi.restoreAllMocks();
	});

	it('disables cancel-all while cancel request is in flight and restores on success', async () => {
		const controller = new StatusPanelRuntime();

		let resolveCancel!: (value: string) => void;
		const inFlightCancel = new Promise<string>((resolve) => {
			resolveCancel = resolve;
		});
		const cancelSpy = vi
			.spyOn(tauriClient, 'cancelProcessing')
			.mockImplementation(() => inFlightCancel);

		const cancelRequest = controller.requestCancelAll();
		expect(cancelSpy).toHaveBeenCalledTimes(1);
		expect(statusPanelViewState.cancelAllPending).toBe(true);

		resolveCancel('cancel requested');
		await cancelRequest;

		expect(statusPanelViewState.cancelAllPending).toBe(false);
		expect(controller.getCurrentStatus().message).toBe('Cancellation requested…');
		expect(getStepText()).toContain('Cancellation requested…');
	});

	it('preserves latest progress when cancel-all resolves after progress advances', async () => {
		const controller = new StatusPanelRuntime();

		let resolveCancel!: (value: string) => void;
		const inFlightCancel = new Promise<string>((resolve) => {
			resolveCancel = resolve;
		});
		vi.spyOn(tauriClient, 'cancelProcessing').mockImplementation(() => inFlightCancel);

		const cancelRequest = controller.requestCancelAll();
		controller.applyProgress({
			operation_kind: 'processingBatch',
			input_index: 0,
			stage: STAGES.writing,
			percentage: 68,
			message: 'Writing metadata',
		});

		resolveCancel('cancel requested');
		await cancelRequest;

		expect(controller.getCurrentStatus()).toMatchObject({
			stage: STAGES.writing,
			percentage: 68,
			message: 'Cancellation requested…',
		});
		expect(getStepText()).toContain('Cancellation requested…');
	});

	it('restores cancel-all enabled state and surfaces explicit error on cancel failure', async () => {
		const controller = new StatusPanelRuntime();
		const consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => undefined);
		vi.spyOn(tauriClient, 'cancelProcessing').mockRejectedValue(
			new Error('tauriClient cancellation failed'),
		);

		await controller.requestCancelAll();

		expect(statusPanelViewState.cancelAllPending).toBe(false);
		expect(getStepText()).toBe('Error: Failed to cancel processing. Please try again.');
		expect(consoleErrorSpy).toHaveBeenCalled();
	});

	it.each([
		{
			name: 'all completed',
			terminalStages: [STAGES.completed, STAGES.completed] as const,
			expectedMethod: 'showSuccess' as const,
			expectedMessage: 'Audiobook created successfully!',
			expectedStepText: 'Audiobook created successfully!',
		},
		{
			name: 'cancelled present',
			terminalStages: [STAGES.cancelled, STAGES.completed] as const,
			expectedMethod: 'showInfo' as const,
			expectedMessage: 'Processing was cancelled.',
			expectedStepText: 'Processing was cancelled.',
		},
		{
			name: 'failed present',
			terminalStages: [STAGES.cancelled, STAGES.failed] as const,
			expectedMethod: 'showError' as const,
			expectedMessage: 'One or more files failed to process.',
			expectedStepText: 'Error: One or more files failed to process.',
		},
	])('applies batch terminal lifecycle reset after 2s when %s', ({
		terminalStages,
		expectedMethod,
		expectedMessage,
		expectedStepText,
	}) => {
		const controller = new StatusPanelRuntime();
		seedDisabledControls();

		const showSuccessSpy = vi.spyOn(feedback, 'showSuccess');
		const showErrorSpy = vi.spyOn(feedback, 'showError');
		const showInfoSpy = vi.spyOn(feedback, 'showInfo');

		controller.applyQueueSnapshot({
			operation_kind: 'processingBatch',
			items: [
				{ input_index: 0, file_path: '/books/alpha.m4b' },
				{ input_index: 1, file_path: '/books/beta.m4b' },
			],
			max_concurrent: 2,
		});

		controller.applyProgress({
			operation_kind: 'processingBatch',
			input_index: 0,
			stage: terminalStages[0],
			percentage: 100,
			message: 'terminal-0',
		});
		controller.applyProgress({
			operation_kind: 'processingBatch',
			input_index: 1,
			stage: terminalStages[1],
			percentage: 100,
			message: 'terminal-1',
		});

		expect(controller.isCurrentlyProcessing).toBe(true);
		expect(getJobRows()).toHaveLength(2);
		assertControlsDisabled();

		vi.advanceTimersByTime(1999);
		expect(controller.isCurrentlyProcessing).toBe(true);

		vi.advanceTimersByTime(1);

		assertControlsEnabled();
		expect(controller.isCurrentlyProcessing).toBe(false);
		const idleStatus = controller.getCurrentStatus();
		expect(idleStatus).toEqual({
			stage: 'idle',
			percentage: 0,
			message: 'Ready to process audiobook',
		});
		expect(idleStatus).not.toHaveProperty('currentFile');
		expect(idleStatus).not.toHaveProperty('etaSeconds');
		expect(statusPanelViewState.jobItems).toHaveLength(0);
		expect(getStepText()).toBe(expectedStepText);

		const toastSpies = {
			showSuccess: showSuccessSpy,
			showError: showErrorSpy,
			showInfo: showInfoSpy,
		};
		const expectedSpy = toastSpies[expectedMethod];
		expect(expectedSpy).toHaveBeenCalledTimes(1);
		expect(expectedSpy).toHaveBeenCalledWith(expectedMessage);
		for (const [method, spy] of Object.entries(toastSpies)) {
			if (method !== expectedMethod) {
				expect(spy).not.toHaveBeenCalled();
			}
		}
	});

	it('reconciles merge skip_existing results back to idle and unlocks controls', () => {
		const controller = new StatusPanelRuntime();
		seedDisabledControls();

		const showInfoSpy = vi.spyOn(feedback, 'showInfo');

		controller.reconcileProcessResult({
			jobType: 'merge',
			summary: { total: 1, succeeded: 0, skipped: 1, cancelled: 0, failed: 0 },
			results: [
				{
					status: 'skipped',
					message: 'Skipped existing output at /books/output.m4b',
				},
			],
		});

		expect(controller.isCurrentlyProcessing).toBe(true);
		expect(getJobRows()).toEqual([
			'Merge output • Skipped existing output at /books/output.m4b (100.0%)',
		]);

		vi.advanceTimersByTime(1499);
		expect(controller.isCurrentlyProcessing).toBe(true);
		assertControlsDisabled();

		vi.advanceTimersByTime(1);

		expect(controller.isCurrentlyProcessing).toBe(false);
		assertControlsEnabled();
		expect(showInfoSpy).toHaveBeenCalledWith('Skipped existing output at /books/output.m4b');
		expect(controller.getCurrentStatus()).toEqual({
			stage: 'idle',
			percentage: 0,
			message: 'Ready to process audiobook',
		});
	});

	it('uses the batch completion override message for partial failures', () => {
		const controller = new StatusPanelRuntime();
		seedDisabledControls();

		const showErrorSpy = vi.spyOn(feedback, 'showError');
		controller.setBatchCompletionMessage('Processed 1/2. Failed: beta.m4b');

		controller.applyQueueSnapshot({
			operation_kind: 'processingBatch',
			items: [
				{ input_index: 0, file_path: '/books/alpha.m4b' },
				{ input_index: 1, file_path: '/books/beta.m4b' },
			],
			max_concurrent: 2,
		});

		controller.applyProgress({
			operation_kind: 'processingBatch',
			input_index: 0,
			stage: STAGES.completed,
			percentage: 100,
			message: 'terminal-0',
		});
		controller.applyProgress({
			operation_kind: 'processingBatch',
			input_index: 1,
			stage: STAGES.failed,
			percentage: 100,
			message: 'terminal-1',
		});

		vi.advanceTimersByTime(2000);

		expect(showErrorSpy).toHaveBeenCalledWith('Processed 1/2. Failed: beta.m4b');
	});

	it('uses an info override when batch results include completed and cancelled entries', () => {
		const controller = new StatusPanelRuntime();
		seedDisabledControls();

		const showSuccessSpy = vi.spyOn(feedback, 'showSuccess');
		const showInfoSpy = vi.spyOn(feedback, 'showInfo');

		controller.applyQueueSnapshot({
			operation_kind: 'processingBatch',
			items: [
				{ input_index: 0, file_path: '/books/alpha.m4b' },
				{ input_index: 1, file_path: '/books/beta.m4b' },
			],
			max_concurrent: 2,
		});

		controller.applyProgress({
			operation_kind: 'processingBatch',
			input_index: 0,
			stage: STAGES.completed,
			percentage: 100,
			message: 'terminal-0',
		});

		controller.reconcileProcessResult({
			jobType: 'batch',
			summary: { total: 2, succeeded: 1, skipped: 0, cancelled: 1, failed: 0 },
			results: [
				{
					inputIndex: 0,
					status: 'success',
					message: 'Successfully created audiobook: /books/alpha.m4b',
				},
				{
					inputIndex: 1,
					status: 'cancelled',
					message: 'Processing was cancelled',
				},
			],
		});
		controller.setBatchCompletionMessage('Processed 1/2. Cancelled: 1.');

		expect(getJobRows()).toEqual(['alpha.m4b • Completed (100.0%)', 'beta.m4b • Cancelled']);

		vi.advanceTimersByTime(2000);

		expect(showInfoSpy).toHaveBeenCalledWith('Processed 1/2. Cancelled: 1.');
		expect(showSuccessSpy).not.toHaveBeenCalled();
		expect(getStepText()).toBe('Processed 1/2. Cancelled: 1.');
	});

	it('preserves skipped batch rows when cancellation arrives after a skipped terminal event', () => {
		const controller = new StatusPanelRuntime();
		seedDisabledControls();

		controller.applyQueueSnapshot({
			operation_kind: 'processingBatch',
			items: [
				{ input_index: 0, file_path: '/books/alpha.m4b' },
				{ input_index: 1, file_path: '/books/beta.m4b' },
			],
			max_concurrent: 2,
		});

		controller.applyProgress({
			operation_kind: 'processingBatch',
			input_index: 0,
			stage: STAGES.skipped,
			percentage: 100,
			message: 'Skipped existing output at /books/alpha.m4b',
		});

		controller.handleProcessingCancellation();

		expect(getJobRows()).toEqual(['alpha.m4b • Skipped (100.0%)', 'beta.m4b • Cancelled']);
	});

	it('shows an informational toast when every batch row is skipped', () => {
		const controller = new StatusPanelRuntime();
		seedDisabledControls();

		const showInfoSpy = vi.spyOn(feedback, 'showInfo');
		const showSuccessSpy = vi.spyOn(feedback, 'showSuccess');
		controller.setBatchCompletionMessage('No files were processed successfully. Skipped: 2.');

		controller.applyQueueSnapshot({
			operation_kind: 'processingBatch',
			items: [
				{ input_index: 0, file_path: '/books/alpha.m4b' },
				{ input_index: 1, file_path: '/books/beta.m4b' },
			],
			max_concurrent: 2,
		});

		controller.applyProgress({
			operation_kind: 'processingBatch',
			input_index: 0,
			stage: STAGES.skipped,
			percentage: 100,
			message: 'Skipped existing output at /books/alpha.m4b',
		});
		controller.applyProgress({
			operation_kind: 'processingBatch',
			input_index: 1,
			stage: STAGES.skipped,
			percentage: 100,
			message: 'Skipped existing output at /books/beta.m4b',
		});

		vi.advanceTimersByTime(2000);

		expect(showInfoSpy).toHaveBeenCalledWith('No files were processed successfully. Skipped: 2.');
		expect(showSuccessSpy).not.toHaveBeenCalled();
		expect(controller.isCurrentlyProcessing).toBe(false);
	});

	it.each([
		{
			name: 'completed',
			stage: STAGES.completed,
			message: 'Audiobook created successfully!',
			method: 'showSuccess' as const,
		},
		{
			name: 'failed',
			stage: STAGES.failed,
			message: 'Encoder failed',
			method: 'showError' as const,
		},
		{
			name: 'cancelled',
			stage: STAGES.cancelled,
			message: 'Processing was cancelled.',
			method: 'showInfo' as const,
		},
	])('applies single-job terminal lifecycle reset after 2s when %s', ({
		stage,
		message,
		method,
	}) => {
		const controller = new StatusPanelRuntime();
		seedDisabledControls();

		const showSuccessSpy = vi.spyOn(feedback, 'showSuccess');
		const showErrorSpy = vi.spyOn(feedback, 'showError');
		const showInfoSpy = vi.spyOn(feedback, 'showInfo');

		controller.applyProgress({
			operation_kind: 'processingBatch',
			job_id: 'job-1',
			stage,
			percentage: 100,
			message,
		});

		expect(controller.isCurrentlyProcessing).toBe(true);
		expect(getJobRows()).toHaveLength(1);
		assertControlsDisabled();

		vi.advanceTimersByTime(1999);
		vi.advanceTimersByTime(1);

		const expectedSpy =
			method === 'showSuccess'
				? showSuccessSpy
				: method === 'showError'
					? showErrorSpy
					: showInfoSpy;
		expect(expectedSpy).toHaveBeenCalledWith(message);
		assertControlsEnabled();
		const idleStatus = controller.getCurrentStatus();
		expect(idleStatus).toEqual({
			stage: 'idle',
			percentage: 0,
			message: 'Ready to process audiobook',
		});
		expect(idleStatus).not.toHaveProperty('currentFile');
		expect(idleStatus).not.toHaveProperty('etaSeconds');
		expect(statusPanelViewState.jobItems).toHaveLength(0);
		expect(getStepText()).toBe(method === 'showError' ? `Error: ${message}` : message);
	});

	it('prevents stale single-job completion timeout from resetting a newer active run', () => {
		const controller = new StatusPanelRuntime();
		seedDisabledControls();

		controller.applyProgress({
			operation_kind: 'processingBatch',
			input_index: 0,
			stage: STAGES.completed,
			percentage: 100,
			message: 'first run done',
		});
		expect(controller.isCurrentlyProcessing).toBe(true);

		vi.advanceTimersByTime(1000);

		controller.applyProgress({
			operation_kind: 'processingBatch',
			input_index: 0,
			stage: STAGES.completed,
			percentage: 100,
			message: 'second run done',
		});
		expect(controller.isCurrentlyProcessing).toBe(true);

		vi.advanceTimersByTime(999);
		expect(controller.isCurrentlyProcessing).toBe(true);
		expect(controller.getCurrentStatus().stage).not.toBe('idle');

		vi.advanceTimersByTime(1);
		expect(controller.isCurrentlyProcessing).toBe(false);
		expect(controller.getCurrentStatus().stage).toBe('idle');
	});

	it('shows cancellation requested before final cancelled summary in batch flow', async () => {
		const controller = new StatusPanelRuntime();
		seedDisabledControls();

		const showSuccessSpy = vi.spyOn(feedback, 'showSuccess');
		const showErrorSpy = vi.spyOn(feedback, 'showError');
		const showInfoSpy = vi.spyOn(feedback, 'showInfo');
		vi.spyOn(tauriClient, 'cancelProcessing').mockResolvedValue('cancel requested');

		controller.applyQueueSnapshot({
			operation_kind: 'processingBatch',
			items: [
				{ input_index: 0, file_path: '/books/alpha.m4b' },
				{ input_index: 1, file_path: '/books/beta.m4b' },
			],
			max_concurrent: 2,
		});

		await controller.requestCancelAll();
		expect(getStepText()).toContain('Cancellation requested…');
		expect(controller.getCurrentStatus().message).toBe('Cancellation requested…');

		controller.applyProgress({
			operation_kind: 'processingBatch',
			input_index: 0,
			stage: STAGES.cancelled,
			percentage: 100,
			message: 'cancelled-0',
		});
		controller.applyProgress({
			operation_kind: 'processingBatch',
			input_index: 1,
			stage: STAGES.cancelled,
			percentage: 100,
			message: 'cancelled-1',
		});

		vi.advanceTimersByTime(2000);

		expect(showSuccessSpy).not.toHaveBeenCalled();
		expect(showErrorSpy).not.toHaveBeenCalled();
		expect(showInfoSpy).toHaveBeenCalledTimes(1);
		expect(showInfoSpy).toHaveBeenCalledWith('Processing was cancelled.');
		expect(getStepText()).toBe('Processing was cancelled.');
	});

	it('synthesizes cancelled completion when command rejection arrives before terminal events', () => {
		const controller = new StatusPanelRuntime();
		seedDisabledControls();

		const showSuccessSpy = vi.spyOn(feedback, 'showSuccess');
		const showErrorSpy = vi.spyOn(feedback, 'showError');
		const showInfoSpy = vi.spyOn(feedback, 'showInfo');

		controller.applyQueueSnapshot({
			operation_kind: 'processingBatch',
			items: [
				{ input_index: 0, file_path: '/books/alpha.m4b' },
				{ input_index: 1, file_path: '/books/beta.m4b' },
			],
			max_concurrent: 2,
		});

		controller.handleProcessingCancellation();
		expect(controller.getCurrentStatus().stage).toBe('cancelled');
		expect(controller.isCurrentlyProcessing).toBe(true);

		vi.advanceTimersByTime(2000);

		expect(showSuccessSpy).not.toHaveBeenCalled();
		expect(showErrorSpy).not.toHaveBeenCalled();
		expect(showInfoSpy).toHaveBeenCalledTimes(1);
		expect(showInfoSpy).toHaveBeenCalledWith('Processing was cancelled.');
		expect(controller.isCurrentlyProcessing).toBe(false);
		expect(getStepText()).toBe('Processing was cancelled.');
	});

	it('retains the batch completion toast message in stepText after resetToIdle', () => {
		const controller = new StatusPanelRuntime();
		seedDisabledControls();

		controller.applyQueueSnapshot({
			operation_kind: 'processingBatch',
			items: [
				{ input_index: 0, file_path: '/books/alpha.m4b' },
				{ input_index: 1, file_path: '/books/beta.m4b' },
			],
			max_concurrent: 2,
		});
		controller.applyProgress({
			operation_kind: 'processingBatch',
			input_index: 0,
			stage: STAGES.completed,
			percentage: 100,
			message: 'terminal-0',
		});
		controller.applyProgress({
			operation_kind: 'processingBatch',
			input_index: 1,
			stage: STAGES.completed,
			percentage: 100,
			message: 'terminal-1',
		});

		vi.advanceTimersByTime(2000);

		// Regression guard: prior to fix, resetToIdle ran AFTER feedback.showSuccess
		// in the same tick, clobbering stepText back to the idle label. The user never
		// saw the success message.
		expect(getStepText()).toBe('Audiobook created successfully!');
		expect(controller.isCurrentlyProcessing).toBe(false);
	});

	it('bypasses progress throttle on stage transitions so mid-job stage changes render immediately', () => {
		const controller = new StatusPanelRuntime();
		seedDisabledControls();

		controller.applyQueueSnapshot({
			operation_kind: 'processingBatch',
			items: [{ input_index: 0, file_path: '/books/alpha.m4b' }],
			max_concurrent: 1,
		});

		controller.applyProgress({
			operation_kind: 'processingBatch',
			input_index: 0,
			stage: 'analyzing',
			percentage: 10,
			message: 'Analyzing audio',
		});
		vi.advanceTimersByTime(20);
		expect(getJobRows()[0]).toContain('(10.0%)');

		// Same job, same tick (< 1s throttle window), non-terminal, but stage changed:
		// must NOT be throttled — user needs to see stage transitions (e.g. writing).
		// Prior to fix, shouldThrottleProgressUpdate dropped this update entirely and
		// the stage transition never reached the UI until the next 1s tick.
		controller.applyProgress({
			operation_kind: 'processingBatch',
			input_index: 0,
			stage: 'writing',
			percentage: 60,
			message: 'Writing output',
		});
		vi.advanceTimersByTime(20);
		expect(getJobRows()[0]).toContain('(60.0%)');
	});

	it('resets queued progress state cleanly before new progress arrives', async () => {
		const controller = new StatusPanelRuntime();

		controller.applyProgress({
			operation_kind: 'processingBatch',
			job_id: 'job-123',
			stage: 'converting',
			percentage: 35,
			message: 'Converting',
		});
		vi.advanceTimersByTime(20);
		expect(getJobRows()).toHaveLength(1);
		expect(getStepText()).toBe('Current Step: Converting');

		controller.resetToIdle();

		const stepAfterReset = getStepText();
		controller.applyProgress({
			operation_kind: 'processingBatch',
			job_id: 'job-123',
			stage: 'converting',
			percentage: 50,
			message: 'Still converting',
		});
		vi.advanceTimersByTime(20);
		expect(getStepText()).not.toBe(stepAfterReset);
		expect(getJobRows()).toHaveLength(1);
		expect(getJobRows()[0]).toContain('(50.0%)');

		controller.resetToIdle();
		controller.applyProgress({
			operation_kind: 'processingBatch',
			job_id: 'job-456',
			stage: 'converting',
			percentage: 42,
			message: 'Converting again',
		});
		vi.advanceTimersByTime(20);

		expect(getJobRows()).toHaveLength(1);
		expect(getJobRows()[0]).toContain('(42.0%)');
	});
});
