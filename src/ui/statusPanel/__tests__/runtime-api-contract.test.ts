import { render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it } from 'vitest';

import * as statusPanel from '..';

const EXPECTED_STATUS_PANEL_EXPORTS = [
	'StatusTransportIsland',
	'initStatusPanel',
	'isStatusPanelProcessing',
	'pushStatusPanelTransientStatus',
	'triggerCancelAllFromStatusPanel',
	'triggerProcessFromStatusPanel',
	'readStatusTransportActive',
] as const;

describe('Status Panel Runtime public API contract', () => {
	beforeEach(() => {
		document.body.innerHTML = '';
	});

	it('pins the status panel public export strip', () => {
		expect(Object.keys(statusPanel).sort()).toEqual([...EXPECTED_STATUS_PANEL_EXPORTS].sort());
	});

	it('renders the foreground transport through the public runtime API', async () => {
		const { container } = render(statusPanel.StatusTransportIsland);
		await tick();

		expect(container.querySelector('[aria-label="Preview transport"]')).toBeTruthy();
	});

	it('reads foreground processing or feedback activity without exposing view state', async () => {
		const { statusPanelViewState, resetStatusPanelViewState } = await import('../viewState.svelte');
		resetStatusPanelViewState();
		expect(statusPanel.readStatusTransportActive()).toBe(false);
		statusPanelViewState.isProcessing = true;
		expect(statusPanel.readStatusTransportActive()).toBe(true);
		statusPanelViewState.isProcessing = false;
		statusPanelViewState.stepColor = 'var(--text-error)';
		expect(statusPanel.readStatusTransportActive()).toBe(true);
	});
});
