import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/svelte';
import JobControlsIsland from '../../jobControls/JobControlsIsland.svelte';
import { tauriClient } from '../../../lib/tauri/client';
import type { ProcessingProgressEvent, ProcessingQueueEvent } from '../../../types/events';
import {
	handleMergeModeChange,
	initJobControls,
	setJobControlsEnabled,
	setJobTypeSelection,
} from '../../jobControls';
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
	});

	afterEach(() => {
		vi.clearAllTimers();
		vi.useRealTimers();
		vi.restoreAllMocks();
	});

	it('clears state and control locks', async () => {
		const controller = new StatusPanelRuntime();
		const mergeToggle = document.getElementById('merge-mode-toggle') as HTMLButtonElement;
		const snapshot: ProcessingQueueEvent = {
			operation_kind: 'processingBatch',
			items: [{ input_index: 0, file_path: '/books/alpha.m4b' }],
			max_concurrent: 1,
		};
		const progress: ProcessingProgressEvent = {
			operation_kind: 'processingBatch',
			input_index: 0,
			stage: 'converting',
			percentage: 25,
			message: 'Working',
		};

		setJobControlsEnabled(false);
		controller.applyQueueSnapshot(snapshot);
		controller.applyProgress(progress);
		vi.advanceTimersByTime(20);

		controller.resetToIdle();

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
		expect(mergeToggle.style.opacity).toBe('1');
	});
});
