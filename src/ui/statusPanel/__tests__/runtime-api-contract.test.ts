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
	'updateStatusPanelConcurrencyStatus',
] as const;

describe('Status Panel Runtime public API contract', () => {
	beforeEach(() => {
		document.body.innerHTML = '';
		statusPanel.updateStatusPanelConcurrencyStatus('');
	});

	it('pins the status panel public export strip', () => {
		expect(Object.keys(statusPanel).sort()).toEqual([...EXPECTED_STATUS_PANEL_EXPORTS].sort());
	});

	it('renders the foreground transport through the public runtime API', async () => {
		const { container } = render(statusPanel.StatusTransportIsland);

		statusPanel.updateStatusPanelConcurrencyStatus('Max jobs: 4');
		await tick();

		expect(container.querySelector('[aria-label="Preview transport"]')).toBeTruthy();
	});
});
