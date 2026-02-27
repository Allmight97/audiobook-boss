import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/svelte';
import JobControlsIsland from '../../jobControls/JobControlsIsland.svelte';
import { tauriClient } from '../../../lib/tauri/client';
import {
	handleMaxConcurrentSelectionChange,
	handleMergeModeChange,
	initJobControls,
	setJobControlsEnabled,
	setJobTypeSelection,
} from '../../jobControls';
import { StatusPanel } from '../logic';
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

	it('clears state/timers and re-enables controls', () => {
		const panel = new StatusPanel();
		const clearTimeoutSpy = vi.spyOn(window, 'clearTimeout');

		const mergeToggle = document.getElementById('merge-mode-toggle') as HTMLInputElement;
		const maxConcurrent = document.getElementById('max-concurrent-select') as HTMLSelectElement;
		setJobControlsEnabled(false);

		(panel as any).handleQueueSnapshot({
			items: [{ input_index: 0, file_path: '/books/alpha.m4b' }],
			max_concurrent: 1,
		});
		panel.updateProgress({
			input_index: 0,
			stage: 'converting',
			percentage: 25,
			message: 'Working',
		} as any);
		vi.advanceTimersByTime(20);

		const progressUnlisten = vi.fn();
		const queueUnlisten = vi.fn();
		(panel as any).progressUnlisten = progressUnlisten;
		(panel as any).queueUnlisten = queueUnlisten;

		const timeoutId = window.setTimeout(() => {}, 10_000);
		(panel as any).batchCompletionTimeout = timeoutId;

		(panel as any).resetToIdle();

		expect(progressUnlisten).toHaveBeenCalledTimes(1);
		expect(queueUnlisten).toHaveBeenCalledTimes(1);
		expect(clearTimeoutSpy).toHaveBeenCalledWith(timeoutId);

		expect(statusPanelViewState.jobItems).toHaveLength(0);
		expect(statusPanelViewState.statusText).toBe('Idle');
		expect(statusPanelViewState.stepText).toBe('Current Step: Ready to process audiobook');
		expect(statusPanelViewState.progressPercentage).toBe(0);
		expect(statusPanelViewState.coverArtDataUrl).toBeNull();
		expect(panel.isCurrentlyProcessing).toBe(false);
		expect(panel.getCurrentStatus()).toEqual({
			stage: 'idle',
			percentage: 0,
			message: 'Ready to process audiobook',
		});

		expect(mergeToggle.disabled).toBe(false);
		expect(maxConcurrent.disabled).toBe(false);
		expect(mergeToggle.style.opacity).toBe('1');
		expect(maxConcurrent.style.opacity).toBe('1');

		// Secondary internal check: cleanup maps are emptied.
		expect((panel as any).jobProgress.size).toBe(0);
		expect((panel as any).lastProgressRenderByKey.size).toBe(0);

		clearTimeoutSpy.mockRestore();
	});
});
