import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/svelte';
import StatusPanelIsland from '../StatusPanelIsland.svelte';
import { resetStatusPanelViewState } from '../viewState.svelte';

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

	it('renders status panel controls and delegates initialization', () => {
		render(StatusPanelIsland);

		expect(document.querySelector('.panel.status-panel')).toBeTruthy();
		const requiredIds = [
			'progress-bar',
			'percentage-processed',
			'status-text',
			'step-text',
			'concurrency-status',
			'process-button',
			'cancel-all-button',
			'job-list',
		];
		requiredIds.forEach((id) => {
			expect(document.getElementById(id)).toBeTruthy();
		});
		expect(initStatusPanelLogicMock).toHaveBeenCalledTimes(1);
	});

	it('wires process and cancel buttons to status-panel actions', () => {
		render(StatusPanelIsland);

		const processButton = document.getElementById('process-button');
		const cancelButton = document.getElementById('cancel-all-button');

		processButton?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
		cancelButton?.dispatchEvent(new MouseEvent('click', { bubbles: true }));

		expect(triggerProcessMock).toHaveBeenCalledTimes(1);
		expect(triggerCancelAllMock).toHaveBeenCalledTimes(1);
	});
});
