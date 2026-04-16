import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/svelte';
import JobControlsIsland from '../../jobControls/JobControlsIsland.svelte';
import { tauriClient } from '../../../lib/tauri/client';
import type { ProcessingProgressEvent, ProcessingQueueEvent } from '../../../types/events';
import {
	handleMaxConcurrentSelectionChange,
	handleMergeModeChange,
	initJobControls,
	setJobControlsEnabled,
	setJobTypeSelection,
} from '../../jobControls';
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

async function flushAsync(): Promise<void> {
	await Promise.resolve();
	await Promise.resolve();
}

describe('StatusPanel resetToIdle', () => {
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

	it('clears state, listeners, and control locks', async () => {
		const controller = new StatusPanelController();
		const mergeToggle = document.getElementById('merge-mode-toggle') as HTMLInputElement;
		const maxConcurrent = document.getElementById('max-concurrent-select') as HTMLSelectElement;
		const snapshot: ProcessingQueueEvent = {
			items: [{ input_index: 0, file_path: '/books/alpha.m4b' }],
			max_concurrent: 1,
		};
		const progress: ProcessingProgressEvent = {
			input_index: 0,
			stage: 'converting',
			percentage: 25,
			message: 'Working',
		};

		setJobControlsEnabled(false);
		await controller.startEventListeners();
		controller.applyQueueSnapshot(snapshot);
		controller.applyProgress(progress);
		vi.advanceTimersByTime(20);

		controller.resetToIdle();

		expect(listenerState.progressUnlisteners).toHaveLength(1);
		expect(listenerState.queueUnlisteners).toHaveLength(1);
		expect(listenerState.progressUnlisteners[0]).toHaveBeenCalledTimes(1);
		expect(listenerState.queueUnlisteners[0]).toHaveBeenCalledTimes(1);
		expect(listenerState.progressCallbacks.size).toBe(0);
		expect(listenerState.queueCallbacks.size).toBe(0);

		expect(statusPanelViewState.jobItems).toHaveLength(0);
		expect(statusPanelViewState.statusText).toBe('Idle');
		expect(statusPanelViewState.stepText).toBe('Current Step: Ready to process audiobook');
		expect(statusPanelViewState.progressPercentage).toBe(0);
		expect(statusPanelViewState.coverArtDataUrl).toBeNull();
		expect(controller.isCurrentlyProcessing).toBe(false);
		const idleStatus = controller.getCurrentStatus();
		expect(idleStatus).toEqual({
			stage: 'idle',
			percentage: 0,
			message: 'Ready to process audiobook',
		});
		expect(idleStatus).not.toHaveProperty('currentFile');
		expect(idleStatus).not.toHaveProperty('etaSeconds');

		expect(mergeToggle.disabled).toBe(false);
		expect(maxConcurrent.disabled).toBe(false);
		expect(mergeToggle.style.opacity).toBe('1');
		expect(maxConcurrent.style.opacity).toBe('1');
	});
});
