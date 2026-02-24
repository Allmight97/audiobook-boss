import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/svelte';
import EncoderPanelIsland from '../encoderPanel/EncoderPanelIsland.svelte';

const { initializeMock } = vi.hoisted(() => ({
	initializeMock: vi.fn(),
}));

vi.mock('../encoderPanel/logic', () => ({
	initializeEncoderPanelLogic: initializeMock,
}));

import { initEncoderPanel } from '../encoderPanel';

describe('EncoderPanel island mount', () => {
	beforeEach(() => {
		initializeMock.mockReset();
	});

	it('mounts encoder controls and runs panel initialization logic', () => {
		render(EncoderPanelIsland);
		initEncoderPanel();

		expect(document.getElementById('encoder-settings-panel')).toBeTruthy();
		expect(document.getElementById('adv-encoder')).toBeTruthy();
		expect(initializeMock).toHaveBeenCalledTimes(1);
	});
});
