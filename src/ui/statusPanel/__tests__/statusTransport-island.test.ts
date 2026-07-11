import { render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import StatusTransportIsland from '../StatusTransportIsland.svelte';
import { resetStatusPanelViewState, statusPanelViewState } from '../viewState.svelte';

const { initStatusPanelLogicMock, triggerCancelAllMock } = vi.hoisted(() => ({
	initStatusPanelLogicMock: vi.fn(() => ({ isCurrentlyProcessing: false })),
	triggerCancelAllMock: vi.fn(),
}));

vi.mock('../controller', () => ({
	StatusPanelRuntime: class {},
	initStatusPanel: initStatusPanelLogicMock,
	pushStatusPanelTransientStatus: vi.fn(),
	triggerCancelAllFromStatusPanel: triggerCancelAllMock,
}));

describe('Status Transport island mount', () => {
	beforeEach(() => {
		resetStatusPanelViewState();
		initStatusPanelLogicMock.mockClear();
		triggerCancelAllMock.mockClear();
	});

	it('renders foreground preview state and delegates initialization', async () => {
		render(StatusTransportIsland);
		statusPanelViewState.progressPercentage = 41.7;
		statusPanelViewState.statusText = 'Previewing';
		await tick();

		expect(document.querySelector('[aria-label="Preview transport"]')).toBeTruthy();
		expect(document.querySelector('[data-testid="status-transport-progress"]')).toHaveStyle({
			width: '41.7%',
		});
		expect(document.body.textContent).toContain('Previewing');
		expect(initStatusPanelLogicMock).toHaveBeenCalledTimes(1);
	});

	it('renders a non-empty backend step message', async () => {
		render(StatusTransportIsland);
		statusPanelViewState.stepText = 'Current Step: Writing chapter metadata';
		await tick();

		expect(document.body.textContent).toContain('Current Step: Writing chapter metadata');
	});

	it('wires Cancel All to the retained local-settle action', async () => {
		render(StatusTransportIsland);
		statusPanelViewState.isProcessing = true;
		statusPanelViewState.jobItems = [
			{
				key: 'processing',
				label: 'Preview.m4b',
				status: 'processing',
				statusText: 'Converting',
				canCancel: true,
				cancelId: 'job-1',
			},
		];
		await tick();

		(document.querySelector('button.btn-pill') as HTMLButtonElement).click();
		expect(triggerCancelAllMock).toHaveBeenCalledTimes(1);
	});
});
