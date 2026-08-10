import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/svelte';
import { tick } from 'svelte';
import StatusPanelIsland from '../StatusPanelIsland.svelte';
import { resetStatusPanelViewState, statusPanelViewState } from '../viewState.svelte';

const { initStatusPanelLogicMock, triggerProcessMock, triggerCancelAllMock } = vi.hoisted(() => ({
	initStatusPanelLogicMock: vi.fn(() => ({ isCurrentlyProcessing: false })),
	triggerProcessMock: vi.fn(),
	triggerCancelAllMock: vi.fn(),
}));

vi.mock('../controller', () => ({
	StatusPanelRuntime: class {},
	initStatusPanel: initStatusPanelLogicMock,
	pushStatusPanelTransientStatus: vi.fn(),
	triggerProcessFromStatusPanel: triggerProcessMock,
	triggerCancelAllFromStatusPanel: triggerCancelAllMock,
}));

describe('StatusPanel island mount', () => {
	beforeEach(() => {
		resetStatusPanelViewState();
		initStatusPanelLogicMock.mockClear();
		triggerProcessMock.mockClear();
		triggerCancelAllMock.mockClear();
	});

	it('initializes the status panel once when rendered', () => {
		render(StatusPanelIsland);

		expect(initStatusPanelLogicMock).toHaveBeenCalledTimes(1);
	});

	it('wires process and cancel buttons to status-panel actions', async () => {
		render(StatusPanelIsland);
		statusPanelViewState.isProcessing = true;
		statusPanelViewState.jobItems = [
			{
				key: 'processing',
				label: 'Book.m4b',
				status: 'processing',
				statusText: 'Converting',
				canCancel: true,
				cancelId: 'job-1',
			},
		];
		await tick();

		const processButton = document.getElementById('process-button');
		const cancelButton = document.getElementById('cancel-all-button');

		processButton?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
		cancelButton?.dispatchEvent(new MouseEvent('click', { bubbles: true }));

		expect(triggerProcessMock).toHaveBeenCalledTimes(1);
		expect(triggerCancelAllMock).toHaveBeenCalledTimes(1);
	});

	it('keeps queue rows collapsed behind visible status chips until requested', async () => {
		const cancelJobMock = vi.fn();
		render(StatusPanelIsland);

		statusPanelViewState.jobItems = [
			{
				key: 'completed',
				label: 'Book 22 - Renegade World.m4b',
				status: 'completed',
				statusText: 'Completed',
				percentage: 100,
				canCancel: false,
			},
			{
				key: 'processing',
				label: 'Book 02 - Dust World.m4b',
				status: 'processing',
				stage: 'writing',
				statusText: 'Writing Metadata',
				percentage: 0,
				canCancel: true,
				cancelId: 'job-1',
				onCancel: cancelJobMock,
			},
			{
				key: 'queued',
				label: 'Book 01 - Steel World.m4b',
				status: 'queued',
				statusText: 'Queued • #3 of 3',
				canCancel: false,
			},
		];
		statusPanelViewState.progressPercentage = 41.7;
		await tick();

		expect(document.getElementById('percentage-processed')?.textContent).toBe('41.7%');
		expect(document.querySelector('[data-testid="queue-chip-active"]')?.textContent).toBe(
			'1 writing',
		);
		expect(document.querySelector('[data-testid="queue-chip-queued"]')?.textContent).toBe(
			'1 queued',
		);
		expect(document.querySelector('[data-testid="queue-chip-complete"]')?.textContent).toBe(
			'1 complete',
		);

		const jobList = document.getElementById('job-list') as HTMLDivElement;
		const toggle = document.getElementById('queue-toggle-button') as HTMLButtonElement;
		expect(toggle.textContent).toContain('View queue');
		expect(toggle.getAttribute('aria-expanded')).toBe('false');
		expect(jobList.hidden).toBe(true);

		toggle.dispatchEvent(new MouseEvent('click', { bubbles: true }));
		await tick();
		expect(toggle.textContent).toContain('Hide queue');
		expect(toggle.getAttribute('aria-expanded')).toBe('true');
		expect(jobList.hidden).toBe(false);
		expect(document.querySelectorAll('.queue-job-row')).toHaveLength(3);

		const cancelButton = document.getElementById('cancel-processing') as HTMLButtonElement;
		cancelButton.dispatchEvent(new MouseEvent('click', { bubbles: true }));
		expect(cancelJobMock).toHaveBeenCalledWith('job-1');

		toggle.dispatchEvent(new MouseEvent('click', { bubbles: true }));
		await tick();
		expect(toggle.textContent).toContain('View queue');
		expect(jobList.hidden).toBe(true);
	});
});
