import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/svelte';
import JobControlsIsland from '../../jobControls/JobControlsIsland.svelte';
import { tauriClient } from '../../../lib/tauri/client';
import {
	STAGES,
	type ProcessingProgressEvent,
	type ProcessingQueueEvent,
} from '../../../types/events';
import {
	handleMaxConcurrentSelectionChange,
	handleMergeModeChange,
	initJobControls,
	setJobControlsEnabled,
	setJobTypeSelection,
} from '../../jobControls';
import * as dom from '../dom';
import { StatusPanelController } from '../controller';
import { resetStatusPanelViewState, statusPanelViewState } from '../viewState.svelte';

const listenerState = vi.hoisted(() => {
	const progressCallbacks = new Set<(event: ProcessingProgressEvent) => void>();
	const queueCallbacks = new Set<(event: ProcessingQueueEvent) => void>();
	const progressUnlisteners: Array<ReturnType<typeof vi.fn>> = [];
	const queueUnlisteners: Array<ReturnType<typeof vi.fn>> = [];

	return {
		progressCallbacks,
		queueCallbacks,
		progressUnlisteners,
		queueUnlisteners,
		listenForProgressEventsMock: vi.fn(
			async (handler: (event: ProcessingProgressEvent) => void) => {
				progressCallbacks.add(handler);
				const unlisten = vi.fn(() => {
					progressCallbacks.delete(handler);
				});
				progressUnlisteners.push(unlisten);
				return unlisten;
			},
		),
		listenForQueueEventsMock: vi.fn(async (handler: (event: ProcessingQueueEvent) => void) => {
			queueCallbacks.add(handler);
			const unlisten = vi.fn(() => {
				queueCallbacks.delete(handler);
			});
			queueUnlisteners.push(unlisten);
			return unlisten;
		}),
	};
});

vi.mock('../events', () => ({
	listenForProgressEvents: listenerState.listenForProgressEventsMock,
	listenForQueueEvents: listenerState.listenForQueueEventsMock,
}));

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

function emitProgressToActiveListeners(event: ProcessingProgressEvent) {
	listenerState.progressCallbacks.forEach((callback) => callback(event));
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

		listenerState.progressCallbacks.clear();
		listenerState.queueCallbacks.clear();
		listenerState.progressUnlisteners.length = 0;
		listenerState.queueUnlisteners.length = 0;
		listenerState.listenForProgressEventsMock.mockClear();
		listenerState.listenForQueueEventsMock.mockClear();
	});

	afterEach(() => {
		vi.clearAllTimers();
		vi.useRealTimers();
		vi.restoreAllMocks();
	});

	it('disables cancel-all while cancel request is in flight and restores on success', async () => {
		const controller = new StatusPanelController();

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

	it('restores cancel-all enabled state and surfaces explicit error on cancel failure', async () => {
		const controller = new StatusPanelController();
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
		},
		{
			name: 'failed present',
			terminalStages: [STAGES.failed, STAGES.completed] as const,
			expectedMethod: 'showError' as const,
			expectedMessage: 'One or more files failed to process.',
		},
		{
			name: 'cancelled present',
			terminalStages: [STAGES.cancelled, STAGES.completed] as const,
			expectedMethod: 'showInfo' as const,
			expectedMessage: 'Processing was cancelled.',
		},
	])('applies batch terminal lifecycle reset after 2s when %s', ({
		terminalStages,
		expectedMethod,
		expectedMessage,
	}) => {
		const controller = new StatusPanelController();
		seedDisabledControls();

		const showSuccessSpy = vi.spyOn(dom, 'showSuccess');
		const showErrorSpy = vi.spyOn(dom, 'showError');
		const showInfoSpy = vi.spyOn(dom, 'showInfo');

		controller.applyQueueSnapshot({
			items: [
				{ input_index: 0, file_path: '/books/alpha.m4b' },
				{ input_index: 1, file_path: '/books/beta.m4b' },
			],
			max_concurrent: 2,
		});

		controller.applyProgress({
			input_index: 0,
			stage: terminalStages[0],
			percentage: 100,
			message: 'terminal-0',
		});
		controller.applyProgress({
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
		expect(controller.getCurrentStatus()).toEqual({
			stage: 'idle',
			percentage: 0,
			message: 'Ready to process audiobook',
		});
		expect(statusPanelViewState.jobItems).toHaveLength(0);
		expect(getStepText()).toBe('Current Step: Ready to process audiobook');

		const expectedSpy =
			expectedMethod === 'showSuccess'
				? showSuccessSpy
				: expectedMethod === 'showError'
					? showErrorSpy
					: showInfoSpy;
		expect(expectedSpy).toHaveBeenCalledWith(expectedMessage);
	});

	it('uses the batch completion override message for partial failures', () => {
		const controller = new StatusPanelController();
		seedDisabledControls();

		const showErrorSpy = vi.spyOn(dom, 'showError');
		controller.setBatchCompletionMessage('Processed 1/2. Failed: beta.m4b');

		controller.applyQueueSnapshot({
			items: [
				{ input_index: 0, file_path: '/books/alpha.m4b' },
				{ input_index: 1, file_path: '/books/beta.m4b' },
			],
			max_concurrent: 2,
		});

		controller.applyProgress({
			input_index: 0,
			stage: STAGES.completed,
			percentage: 100,
			message: 'terminal-0',
		});
		controller.applyProgress({
			input_index: 1,
			stage: STAGES.failed,
			percentage: 100,
			message: 'terminal-1',
		});

		vi.advanceTimersByTime(2000);

		expect(showErrorSpy).toHaveBeenCalledWith('Processed 1/2. Failed: beta.m4b');
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
		const controller = new StatusPanelController();
		seedDisabledControls();

		const showSuccessSpy = vi.spyOn(dom, 'showSuccess');
		const showErrorSpy = vi.spyOn(dom, 'showError');
		const showInfoSpy = vi.spyOn(dom, 'showInfo');

		controller.applyProgress({
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
		expect(controller.getCurrentStatus()).toEqual({
			stage: 'idle',
			percentage: 0,
			message: 'Ready to process audiobook',
		});
		expect(statusPanelViewState.jobItems).toHaveLength(0);
		expect(getStepText()).toBe(method === 'showError' ? `Error: ${message}` : message);
	});

	it('prevents stale single-job completion timeout from resetting a newer active run', () => {
		const controller = new StatusPanelController();
		seedDisabledControls();

		controller.applyProgress({
			input_index: 0,
			stage: STAGES.completed,
			percentage: 100,
			message: 'first run done',
		});
		expect(controller.isCurrentlyProcessing).toBe(true);

		vi.advanceTimersByTime(1000);

		controller.applyProgress({
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
		const controller = new StatusPanelController();
		seedDisabledControls();

		const showInfoSpy = vi.spyOn(dom, 'showInfo');
		vi.spyOn(tauriClient, 'cancelProcessing').mockResolvedValue('cancel requested');

		controller.applyQueueSnapshot({
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
			input_index: 0,
			stage: STAGES.cancelled,
			percentage: 100,
			message: 'cancelled-0',
		});
		controller.applyProgress({
			input_index: 1,
			stage: STAGES.cancelled,
			percentage: 100,
			message: 'cancelled-1',
		});

		vi.advanceTimersByTime(2000);

		expect(showInfoSpy).toHaveBeenCalledWith('Processing was cancelled.');
		expect(getStepText()).toBe('Current Step: Ready to process audiobook');
	});

	it('cleans up listeners on reset and restarts without duplicate active handlers', async () => {
		const controller = new StatusPanelController();

		await controller.startEventListeners();
		expect(listenerState.progressCallbacks.size).toBe(1);
		expect(listenerState.queueCallbacks.size).toBe(1);

		emitProgressToActiveListeners({
			job_id: 'job-123',
			stage: 'converting',
			percentage: 35,
			message: 'Converting',
		});
		vi.advanceTimersByTime(20);
		expect(getJobRows()).toHaveLength(1);
		expect(getStepText()).toBe('Current Step: Converting');

		controller.resetToIdle();
		expect(listenerState.progressUnlisteners[0]).toHaveBeenCalledTimes(1);
		expect(listenerState.queueUnlisteners[0]).toHaveBeenCalledTimes(1);
		expect(listenerState.progressCallbacks.size).toBe(0);
		expect(listenerState.queueCallbacks.size).toBe(0);

		const stepAfterReset = getStepText();
		emitProgressToActiveListeners({
			job_id: 'job-123',
			stage: 'converting',
			percentage: 50,
			message: 'Still converting',
		});
		vi.advanceTimersByTime(20);
		expect(getStepText()).toBe(stepAfterReset);

		await controller.startEventListeners();
		expect(listenerState.listenForProgressEventsMock).toHaveBeenCalledTimes(2);
		expect(listenerState.listenForQueueEventsMock).toHaveBeenCalledTimes(2);
		expect(listenerState.progressCallbacks.size).toBe(1);
		expect(listenerState.queueCallbacks.size).toBe(1);

		emitProgressToActiveListeners({
			job_id: 'job-456',
			stage: 'converting',
			percentage: 42,
			message: 'Converting again',
		});
		vi.advanceTimersByTime(20);

		expect(getJobRows()).toHaveLength(1);
		expect(getJobRows()[0]).toContain('(42.0%)');

		controller.resetToIdle();
		expect(listenerState.progressUnlisteners[1]).toHaveBeenCalledTimes(1);
		expect(listenerState.queueUnlisteners[1]).toHaveBeenCalledTimes(1);
	});
});
